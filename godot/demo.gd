# The watchable demo: a pack discharging in real time, with its signals on screen.
#
# Run:  godot --path godot
#
# Everything else in this project is a check. This is the one thing you look at, and it
# exists because a phase whose deliverable is "a node you can put in a game" should be
# demonstrable by putting it in a scene rather than only by asserting about it.
#
# # Why the data lives in res://assets
# The node takes scenario and chemistry **text**, never paths, because `res://` resolves
# inside a `.pck` once a game is exported — a path-taking node would work in the editor and
# fail in a shipped build. So this scene does what a shipped game does: it bundles its data
# under `res://` and reads it with `FileAccess`.
#
# Those two files are copies of the repo's canonical `scenarios/` and `chemistries/`
# entries. The copy cannot drift: `crates/sim-godot/tests/demo_assets.rs` asserts they are
# byte-identical, and fails the ordinary `cargo test --workspace` if they are not.

extends Control

const SCENARIO := "res://assets/cc_discharge_lfp.toml"
const CHEMISTRY := "res://assets/lfp_26650_generic.toml"

@onready var _pack: Node = $Pack
@onready var _readout: Label = $Readout
@onready var _log: Label = $EventLog

var _events: Array[String] = []


func _ready() -> void:
	var scenario := FileAccess.get_file_as_string(SCENARIO)
	var chemistry := FileAccess.get_file_as_string(CHEMISTRY)
	if scenario.is_empty() or chemistry.is_empty():
		_readout.text = "Could not read the demo's data from res://assets."
		return

	if not _pack.load_scenario(scenario, chemistry):
		_readout.text = "load_scenario failed:\n%s" % _pack.last_error()
		return

	# Every signal the node can emit, so the demo shows the whole surface rather than the
	# two that happen to fire on this scenario.
	_pack.soc_changed.connect(_on_soc_changed)
	_pack.protection_tripped.connect(_on_protection_tripped)
	_pack.protection_cleared.connect(_on_protection_cleared)
	_pack.thermal_runaway_started.connect(func(): _note("THERMAL RUNAWAY"))
	_pack.vented.connect(func(): _note("VENTED"))
	_pack.contactor_opened.connect(func(): _note("contactor opened"))
	_pack.contactor_closed.connect(func(): _note("contactor closed"))
	_pack.falling_behind.connect(_on_falling_behind)

	# A 1 C discharge on a 2.5 Ah cell — an hour of battery life, watchable in about
	# twenty seconds.
	#
	# `speed` is what makes it fast; `fixed_dt` is NOT. Raising fixed_dt makes each step
	# cover more simulated time and makes the accumulator take proportionally fewer of
	# them, so the pack still advances one simulated second per wall second. (This comment
	# said "fixed_dt = 100x" before a headless probe showed sim time tracking wall time
	# exactly. Worth leaving the correction visible — it is an easy one to get backwards.)
	#
	# The frame rate still defines nothing: every step is exactly `fixed_dt`, and a
	# fast-forward is bit-identical to a real-time run of the same step count.
	_pack.fixed_dt = 1.0
	_pack.speed = 180.0
	_pack.max_steps_per_frame = 32
	_pack.demand_json = '{"Current": 2.5}'
	_pack.auto_step = true
	_note("running: 2.5 A discharge, fixed_dt = %.2f s, speed = %dx"
		% [_pack.fixed_dt, int(_pack.speed)])


func _process(_delta: float) -> void:
	if not _pack.has_scenario():
		return
	_readout.text = "\n".join([
		"sim time      %8.1f s" % _pack.sim_time_s(),
		"terminal      %8.4f V" % _pack.v_terminal(),
		"current       %8.4f A  (positive = discharge)" % _pack.i_actual(),
		"SOC (truth)   %8.4f" % _pack.soc_true(),
		"SOC (BMS)     %8s" % ("--" if is_nan(_pack.soc_bms()) else "%.4f" % _pack.soc_bms()),
		"T min / max   %8.2f / %.2f K" % [_pack.t_min(), _pack.t_max()],
		"SOH capacity  %8.4f" % _pack.soh_capacity(),
		"flags         %8s" % ("--" if _pack.flags_text().is_empty() else _pack.flags_text()),
		"",
		"topology      %dS%dP     BMS: %s" % [
			_pack.series(), _pack.parallel(), "yes" if _pack.has_bms() else "no"],
		"carried       %8.4f s (accumulator remainder)" % _pack.pending_s(),
	])


func _on_soc_changed(soc: float) -> void:
	# Fires on a threshold, not every frame — SOC changes every step, so an unconditioned
	# signal would be a 60 Hz signal. See `soc_signal_epsilon`.
	if _events.size() < 2 or not _events[-1].begins_with("SOC "):
		_note("SOC %.3f" % soc)
	else:
		_events[-1] = "SOC %.3f" % soc
		_refresh_log()


func _on_protection_tripped(_bits: int, text: String) -> void:
	_note("PROTECTION: %s" % text)


func _on_protection_cleared(_bits: int) -> void:
	_note("protection cleared")


func _on_falling_behind(backlog_s: float) -> void:
	_note("falling behind by %.3f s of simulated time" % backlog_s)


func _note(message: String) -> void:
	_events.append(message)
	if _events.size() > 12:
		_events.pop_front()
	_refresh_log()


func _refresh_log() -> void:
	_log.text = "\n".join(_events)
