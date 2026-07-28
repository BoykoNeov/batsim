# The Godot leg of the Phase 5 exit gate.
#
# Run by `crates/sim-godot/tests/godot_gate.rs`; not meant to be run by hand.
#
#   godot --headless --path godot --script gate.gd -- <repo-root> <scenario> <schedule.json>
#
# This script's entire job is: **drive the schedule, print bit patterns, quit.** Every
# decision, every comparison and every assertion lives in Rust. Three reasons, all of them
# measured rather than assumed (see docs/plans/phase-5-godot.md):
#
#  1. GDScript cannot print a float without losing bits. `str(0.7995885912375074)` gives
#     `0.79958859123751`, which does not parse back equal. So numbers leave here as the
#     little-endian bytes of the IEEE-754 f64, hex-encoded — never as decimal text.
#  2. A failing `assert()` abandons the enclosing function without reaching `quit()`, and
#     a headless SceneTree then runs forever. There is no `assert` in this file, and every
#     path ends at an explicit `quit(code)`.
#  3. Comparison belongs where `f64::to_bits` and real assertion semantics live.
#
# The schedule arrives as JSON written by the Rust side, so both legs run **one**
# definition of the experiment rather than two that could drift apart.

extends SceneTree


func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() < 3:
		print("GATE ERROR: expected <repo-root> <scenario> <schedule.json>")
		quit(2)
		return
	var root: String = args[0]
	var scenario_name: String = args[1]
	var schedule_path: String = args[2]

	if not ClassDB.class_exists("BatteryPack"):
		print("GATE ERROR: BatteryPack is not registered — is the cdylib built?")
		quit(2)
		return

	var scenario := _read(root + "/scenarios/" + scenario_name)
	if scenario.is_empty():
		quit(2)
		return
	var schedule_text := _read(schedule_path)
	if schedule_text.is_empty():
		quit(2)
		return
	var schedule: Variant = JSON.parse_string(schedule_text)
	if typeof(schedule) != TYPE_ARRAY:
		print("GATE ERROR: schedule is not a JSON array")
		quit(2)
		return

	var pack := BatteryPack.new()
	var id: String = pack.chemistry_id_of(scenario)
	var chem := ""
	if not id.is_empty():
		chem = _read(root + "/chemistries/" + id + ".toml")
	if not pack.load_scenario(scenario, chem):
		print("GATE ERROR: load_scenario: ", pack.last_error())
		quit(2)
		return

	# The reading that exists before anything runs, so the two legs agree about the
	# starting point and not only about where they ended up.
	_emit(pack)

	for leg in schedule:
		match leg["op"]:
			"step":
				if not pack.step_batch(leg["dt"], int(leg["n"]), leg["demand"]):
					print("GATE ERROR: step_batch: ", pack.last_error())
					quit(2)
					return
				_emit(pack)
			"snapshot_restore":
				# Round-trips the whole engine state through a GDScript String and back.
				# Inside the bit-identical claim on purpose: this is where a lossy float
				# encode would show up, and it is the leg Phase 4 proved was worth having.
				var snap: String = pack.snapshot_json()
				if snap.is_empty():
					print("GATE ERROR: snapshot_json: ", pack.last_error())
					quit(2)
					return
				if not pack.restore_json(snap):
					print("GATE ERROR: restore_json: ", pack.last_error())
					quit(2)
					return
				_emit(pack)
			_:
				print("GATE ERROR: unknown op ", leg["op"])
				quit(2)
				return

	pack.free()
	print("GATE DONE")
	quit(0)


# One sample, as bits.
#
# The floats travel as one PackedFloat64Array so the byte order is the engine's own and
# there is no per-field formatting to get wrong. Field order is fixed and must match
# `godot_gate.rs`'s `Sample`.
func _emit(pack: Node) -> void:
	var values := PackedFloat64Array([
		pack.sim_time_s(),
		pack.v_terminal(),
		pack.i_actual(),
		pack.soc_true(),
		pack.t_min(),
		pack.t_max(),
		pack.soh_capacity(),
	])
	print("SAMPLE ", pack.flags_bits(), " ", values.to_byte_array().hex_encode())


func _read(path: String) -> String:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		print("GATE ERROR: cannot read ", path, " (error ", FileAccess.get_open_error(), ")")
		return ""
	var text := f.get_as_text()
	f.close()
	return text
