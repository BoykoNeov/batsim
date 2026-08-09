"""Print a TOML-ready [dfn] block for a batsim chemistry, extracted from PyBaMM.

The sibling of `extract_spm.py`, and the same verb for the same reason: these are values
a parameter set publishes, not curves fitted to its output, so each one carries a literal
citation instead of an order-of-magnitude apology.

    python tools/reference/extract_dfn.py nmc_21700_lgm50

Paste the emitted block into the matching `chemistries/*.toml`, after `[spm]` — `[dfn]`
**extends** it and does not stand alone. Not shipped; requires PyBaMM (see
requirements.txt).

## The two transport properties are the whole difficulty, and the answer is a good one

`ParameterValues("Chen2020")` returns **callables** for `Electrolyte diffusivity` and
`Electrolyte conductivity`. That is the shape that made `extract_spm.py` parse function
bodies for `m_ref` and `E_r`. The obvious resolution here — sample onto a grid and
document the interpolation error, as the OCP tables do — turns out to be unnecessary and
would be actively harmful: Phase 6 found the OCP tables' own 1.90/1.88 mV interpolation
error *was* the SPM's accuracy floor, so a second sampled table would raise that floor
for nothing.

Underneath, both callables are published closed-form fits (Nyman 2008, LiPF6 in EC:EMC
3:7) with **no temperature dependence at all** — the source says so in a comment:

    D_c_e   = 8.794e-11*(c/1000)**2 - 3.972e-10*(c/1000) + 4.862e-10        # m2/s
    sigma_e = 0.1297*(c/1000)**3 - 2.51*(c/1000)**1.5 + 3.329*(c/1000)      # S/m

So they are stored **exactly**, as (coefficient, exponent) pairs in the fit's own
variable `x = c_e/1000`, and carry no interpolation error. Note the `**1.5`: this is a
sum of power terms, not a polynomial, which is why the schema is a pair and not a
coefficient array.

## How the coefficients are read, and why this is not a regex

`extract_spm.py` reads scalar literals with a regex, which works there because they are
`name = value` assignments. These are **expressions**, so the same approach cannot reach
them. This module parses the function's source with `ast` and destructures the assignment
into terms, then **evaluates the parsed terms against PyBaMM's own callable at several
concentrations** and refuses to emit anything if they disagree. That numeric cross-check
is what makes a silent mis-parse loud: a shape change PyBaMM makes upstream raises here
instead of quietly shipping a wrong electrolyte.
"""

from __future__ import annotations

import ast
import inspect
import sys
import textwrap

import numpy as np
import pybamm

from common import PARAM_SETS

# Concentrations [mol/m3] the parsed terms are cross-checked against PyBaMM's callable.
#
# Spread over the range a 3C discharge actually visits on this set rather than clustered
# near the 1000 mol/m3 reference: the spike measured c_e reaching ~0 in the positive
# electrode and ~3160 at the peak, and both fits are non-monotone in between, so a check
# at one point would pass on a fit that had a term dropped.
CHECK_CONCENTRATIONS = [1.0, 100.0, 500.0, 1000.0, 1500.0, 2200.0, 3200.0]

# Agreement required between the parsed terms and PyBaMM's own evaluation, relative.
# Tight because this is not an approximation of anything: it is the same arithmetic in
# the same order, so the only thing separating the two is the last bit or two.
CHECK_RTOL = 1e-12


def _terms_from_expression(node, path: str):
    """Destructure `a*x**p + b*x**q - c*x` into `[(coefficient, exponent), ...]`.

    `x` must be the sub-expression `c_e / 1000` — the fit's own variable. Anything this
    cannot account for raises, because a term silently dropped here is an electrolyte
    that is wrong by exactly that term and wrong in a way no test downstream would name.
    """
    if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub)):
        left = _terms_from_expression(node.left, path)
        right = _terms_from_expression(node.right, path)
        if isinstance(node.op, ast.Sub):
            right = [(-c, e) for c, e in right]
        return left + right

    # A bare constant is a valid term with exponent 0.
    if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
        return [(float(node.value), 0.0)]

    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Mult):
        # coefficient * (variable ** exponent), or coefficient * variable.
        coeff_node, rest = node.left, node.right
        if not isinstance(coeff_node, ast.Constant):
            coeff_node, rest = node.right, node.left
        if not isinstance(coeff_node, ast.Constant):
            raise RuntimeError(f"{path}: product with no constant factor: {ast.dump(node)}")
        coefficient = float(coeff_node.value)
        if isinstance(rest, ast.BinOp) and isinstance(rest.op, ast.Pow):
            _assert_is_variable(rest.left, path)
            if not isinstance(rest.right, ast.Constant):
                raise RuntimeError(f"{path}: non-constant exponent")
            return [(coefficient, float(rest.right.value))]
        _assert_is_variable(rest, path)
        return [(coefficient, 1.0)]

    raise RuntimeError(
        f"{path}: unrecognised term shape {ast.dump(node)}. PyBaMM's fit has changed "
        f"form and this extractor needs updating — do NOT paper over it by sampling "
        f"the callable onto a grid, which would reintroduce an interpolation floor."
    )


def _assert_is_variable(node, path: str) -> None:
    """The variable must be exactly `c_e / 1000`, the form the published fit uses."""
    ok = (
        isinstance(node, ast.BinOp)
        and isinstance(node.op, ast.Div)
        and isinstance(node.left, ast.Name)
        and isinstance(node.right, ast.Constant)
        and float(node.right.value) == 1000.0
    )
    if not ok:
        raise RuntimeError(
            f"{path}: expected the variable to be `c_e / 1000`, got {ast.dump(node)}. "
            f"The schema stores coefficients in that variable, so a different scaling "
            f"would make every shipped coefficient wrong by a power of it."
        )


def _fit_terms(pv, key: str, assigned_to: str):
    """Parse one transport callable into terms, then verify them numerically."""
    fn = pv[key]
    # `textwrap.dedent`, not `inspect.cleandoc`: cleandoc strips the leading indentation
    # of the *body* relative to the first line, which turns a module-level `def` into a
    # function with an unindented block and an IndentationError.
    tree = ast.parse(textwrap.dedent(inspect.getsource(fn)))
    target = None
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign):
            names = [t.id for t in node.targets if isinstance(t, ast.Name)]
            if assigned_to in names:
                target = node.value
    if target is None:
        raise RuntimeError(
            f"{key}: no assignment to '{assigned_to}' in {fn.__name__}; PyBaMM's "
            f"parameter set has changed shape and this extractor needs updating"
        )
    terms = _terms_from_expression(target, key)

    # The cross-check. `pv.evaluate` is the same path the model itself would take, so
    # agreement here means the stored terms ARE the parameter set's function.
    for c in CHECK_CONCENTRATIONS:
        theirs = float(pv.evaluate(fn(pybamm.Scalar(c), pybamm.Scalar(298.15))))
        ours = sum(coef * (c / 1000.0) ** exp for coef, exp in terms)
        if not np.isclose(ours, theirs, rtol=CHECK_RTOL, atol=0.0):
            raise RuntimeError(
                f"{key}: parsed terms disagree with PyBaMM at c_e = {c} mol/m3 "
                f"({ours!r} vs {theirs!r}). The parse is wrong, or the fit changed."
            )
    return terms


def _fmt_terms(terms) -> str:
    inner = ", ".join(
        f"{{ coefficient = {c!r}, exponent = {e:g} }}" for c, e in terms
    )
    return f"[{inner}]"


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in PARAM_SETS:
        ids = ", ".join(sorted(PARAM_SETS))
        print(f"usage: python extract_dfn.py <chem_id>   (one of: {ids})", file=sys.stderr)
        return 2

    param_set = PARAM_SETS[argv[1]]
    pv = pybamm.ParameterValues(param_set)

    d_terms = _fit_terms(pv, "Electrolyte diffusivity [m2.s-1]", "D_c_e")
    k_terms = _fit_terms(pv, "Electrolyte conductivity [S.m-1]", "sigma_e")

    print(f"# --- extracted from PyBaMM {param_set} (pybamm {pybamm.__version__}) "
          f"by tools/reference/extract_dfn.py ---")
    print("# Every value below is a key of the parameter set or a coefficient of one of "
          "its published")
    print("# closed-form electrolyte fits, read out of the function source and "
          "cross-checked against")
    print(f"# PyBaMM's own evaluation at {len(CHECK_CONCENTRATIONS)} concentrations. "
          f"Nothing here is fitted, sampled or invented.")
    print()
    print("[dfn]")
    print(f"transference_number  = {pv['Cation transference number']!r}   "
          f"# \"Cation transference number\"")
    print(f"thermodynamic_factor = {pv['Thermodynamic factor']!r}   "
          f"# \"Thermodynamic factor\" - literally 1.0, so the (1 + dlnf/dlnc) "
          f"term is unity")
    print("# D_e(c_e) [m2/s] and kappa_e(c_e) [S/m] as sums of coefficient * "
          "(c_e/1000)^exponent.")
    print("# Nyman 2008, LiPF6 in EC:EMC 3:7; no temperature dependence, per the fit's "
          "own comment.")
    print(f"electrolyte_diffusivity_terms  = {_fmt_terms(d_terms)}")
    print(f"electrolyte_conductivity_terms = {_fmt_terms(k_terms)}")
    print()

    for name, side in [("negative", "Negative"), ("positive", "Positive")]:
        print(f"[dfn.{name}]")
        print(f"porosity                   = {pv[f'{side} electrode porosity']!r}   "
              f"# \"{side} electrode porosity\" (eps_e, NOT the [spm] "
              f"active_volume_fraction)")
        print(f"bruggeman_electrolyte      = "
              f"{pv[f'{side} electrode Bruggeman coefficient (electrolyte)']!r}")
        print(f"bruggeman_electrode        = "
              f"{pv[f'{side} electrode Bruggeman coefficient (electrode)']!r}   "
              f"# the set's own value, not an omission")
        print(f"solid_conductivity_s_per_m = "
              f"{pv[f'{side} electrode conductivity [S.m-1]']!r}")
        print()

    print("[dfn.separator]")
    print(f"thickness_m           = {pv['Separator thickness [m]']:.6g}   "
          f"# \"Separator thickness [m]\"")
    print(f"porosity              = {pv['Separator porosity']!r}   # \"Separator porosity\"")
    print(f"bruggeman_electrolyte = "
          f"{pv['Separator Bruggeman coefficient (electrolyte)']!r}")
    print()
    print(f"# For reference, the initial electrolyte concentration is "
          f"{pv['Initial concentration in electrolyte [mol.m-3]']:.6g} mol/m3 and is NOT")
    print(f"# repeated here: it is [spm].c_e_mol_per_m3, which an SPM holds CONSTANT and "
          f"a DFN takes")
    print(f"# as the initial value of a field it then solves for. Same key, same number, "
          f"two readings.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
