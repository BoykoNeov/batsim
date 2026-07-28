# Headless smoke check for the BatteryPack node.
#
# Run:  godot --headless --path godot --script smoke.gd -- <repo-root>
#
# This is NOT the exit gate — that is slice D, lives in Rust, and compares bit patterns.
# This script answers a different and narrower question: **is the node wired up at all?**
#
# In particular it is the ONLY end-to-end check of the accumulator. The exit gate drives
# `step_batch`, the explicit path, because that is the only path whose trajectory is
# reproducible enough to assert bit-identity on. So the accumulator gets thorough unit
# tests in `crates/sim-godot/tests/driver.rs` and would otherwise have *no* evidence that
# `_physics_process` is connected to it. That check is below, and it should not be
# deleted without moving it somewhere.
#
# No `assert()` anywhere: a failing GDScript assert abandons the enclosing function
# without reaching `quit()`, and a headless SceneTree then runs forever. Measured, not
# guessed — see docs/plans/phase-5-godot.md. Every path here ends at an explicit quit().

extends SceneTree

var _failures: Array[String] = []
var _root: String = ""

# Accumulator probe state.
var _pack: Node = null
var _frames := 0
var _signals := {}


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.is_empty():
		_fail("pass the repo root after `--`")
		_finish()
		return
	_root = args[0]

	var scenario := _read("%s/scenarios/cc_discharge_lfp.toml" % _root)
	if scenario.is_empty():
		_finish()
		return

	if not ClassDB.class_exists("BatteryPack"):
		_fail("BatteryPack is not registered — is the cdylib built?")
		_finish()
		return

	var pack := BatteryPack.new()

	# --- construction, and the reading that must exist before anything runs -----------
	var id: String = pack.chemistry_id_of(scenario)
	if id.is_empty():
		_fail("chemistry_id_of returned nothing: %s" % pack.last_error())
		_finish()
		return
	var chem := _read("%s/chemistries/%s.toml" % [_root, id])
	if not pack.load_scenario(scenario, chem):
		_fail("load_scenario failed: %s" % pack.last_error())
		_finish()
		return

	if pack.sim_time_s() != 0.0:
		_fail("a fresh pack reports sim_time_s = %f" % pack.sim_time_s())
	# The priming step exists so this is a measurement rather than a default.
	if not (pack.v_terminal() > 1.0):
		_fail("a fresh pack reports v_terminal = %f, which is a default not a reading"
			% pack.v_terminal())

	# --- the explicit path -------------------------------------------------------------
	if not pack.step_batch(1.0, 100, '{"Current": 2.0}'):
		_fail("step_batch failed: %s" % pack.last_error())
	if absf(pack.sim_time_s() - 100.0) > 1e-9:
		_fail("100 steps of dt=1.0 gave sim_time_s = %f" % pack.sim_time_s())

	# --- snapshot round trip through GDScript strings ----------------------------------
	var snap: String = pack.snapshot_json()
	if snap.is_empty():
		_fail("snapshot_json returned nothing: %s" % pack.last_error())
	elif not pack.restore_json(snap):
		_fail("restore_json failed: %s" % pack.last_error())

	# --- errors are reported, not swallowed --------------------------------------------
	if pack.step_batch(1.0, 1, '{"nonsense": 1}'):
		_fail("a malformed demand was accepted")
	elif pack.last_error().is_empty():
		_fail("a rejected demand left last_error empty")

	# --- the BMS toggle ------------------------------------------------------------------
	if not pack.restart(pack.has_bms()):
		_fail("restart failed: %s" % pack.last_error())
	if pack.sim_time_s() != 0.0:
		_fail("restart left sim_time_s = %f" % pack.sim_time_s())

	pack.free()

	# --- the accumulator, driven by real physics frames ---------------------------------
	# Everything above could have been done without Godot running. This part cannot: it
	# checks that `_physics_process` actually reaches the accumulator, which no Rust test
	# and no part of the exit gate can see.
	_pack = BatteryPack.new()
	_pack.scenario_toml = scenario
	_pack.chemistry_toml = chem
	_pack.fixed_dt = 0.02
	_pack.max_steps_per_frame = 64
	_pack.soc_signal_epsilon = 1e-9  # fire readily, so absence of the signal means absence
	_pack.demand_json = '{"Current": 2.0}'
	# 30 physics frames is about 0.5 s of wall time, so at speed 1.0 the pack could not
	# possibly pass ~0.5 s of simulated time. Asking for 10x makes the check below
	# *discriminating*: a `speed` that was ignored, or applied to `dt` instead of to the
	# time fed in, both fail it.
	_pack.speed = 10.0
	if not _pack.load_from_exports():
		_fail("load_from_exports failed: %s" % _pack.last_error())
		_finish()
		return
	for name in ["soc_changed", "flags_changed", "falling_behind", "protection_tripped"]:
		_pack.connect(name, Callable(self, "_on_signal").bind(name))
	_pack.auto_step = true
	root.add_child(_pack)


# Godot calls this once per physics frame; `_initialize` cannot wait for frames itself.
func _physics_process(delta: float) -> bool:
	_frames += 1
	if _frames < 30:
		return false

	var elapsed: float = _pack.sim_time_s()
	# 30 frames at the default 60 Hz tick is ~0.5 s of wall time, scaled by speed = 10 and
	# consumed in 0.02 s steps — so roughly 5 s of simulated time. The exact count depends
	# on frame timing, which is precisely why the exit gate does not drive this path, so
	# the assertions are a whole number of steps and a sane band rather than an exact value.
	if elapsed <= 0.0:
		_fail("after %d physics frames the accumulator advanced nothing" % _frames)
	else:
		var steps: float = elapsed / 0.02
		if absf(steps - roundf(steps)) > 1e-6:
			_fail("sim_time_s = %.17f is not a whole number of 0.02 s steps" % elapsed)
		# Below 1 s means speed was ignored: ~0.5 s of wall time is all that elapsed.
		if elapsed < 1.0:
			_fail("30 frames at speed 10 gave only %f s of simulated time — `speed` is \
not scaling the time fed to the accumulator" % elapsed)
		if elapsed > 30.0:
			_fail("30 frames consumed %f s of simulated time — far more than speed 10 \
explains" % elapsed)
	if _pack.pending_s() >= 0.02:
		_fail("the accumulator is carrying %f s, more than a whole step"
			% _pack.pending_s())
	if not _signals.has("soc_changed"):
		_fail("30 physics frames of 2 A discharge emitted no soc_changed")

	_finish()
	return true


func _on_signal(a = null, b = null, c = null) -> void:
	# One handler for every signal; the bound name arrives last. Signals here have 0-2
	# arguments, so the name lands in whichever slot is next.
	var name: Variant = c if c != null else (b if b != null else a)
	_signals[name] = true


func _read(path: String) -> String:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		_fail("cannot read %s (error %d)" % [path, FileAccess.get_open_error()])
		return ""
	var text := f.get_as_text()
	f.close()
	return text


func _fail(message: String) -> void:
	_failures.append(message)


func _finish() -> void:
	if _failures.is_empty():
		print("SMOKE OK")
		quit(0)
	else:
		for message in _failures:
			print("SMOKE FAIL: ", message)
		quit(1)
