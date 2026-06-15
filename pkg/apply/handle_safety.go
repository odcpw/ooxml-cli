package apply

import (
	"fmt"

	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
)

// StructuralShiftCommand reports whether command inserts/deletes XLSX rows or
// columns, changing A1 addresses in the sheet grid.
func StructuralShiftCommand(command string) bool {
	switch NormalizeCommand(command) {
	case "xlsx rows insert", "xlsx rows delete", "xlsx cols insert", "xlsx cols delete":
		return true
	default:
		return false
	}
}

// FirstAddressPositionalArg returns the first op arg carrying an XLSX
// address-positional handle (cell/comment handles whose A1 address can become
// stale after row/column structural edits).
func FirstAddressPositionalArg(op Operation) (key string, value string, ok bool) {
	for k, arg := range op.Args {
		if xlsxhandle.IsAddressPositional(arg.String()) {
			return k, arg.String(), true
		}
	}
	return "", "", false
}

// HandleSafetyViolation describes a statically detected op sequence that could
// target the wrong XLSX cell/comment after a row/column structural edit.
type HandleSafetyViolation struct {
	OpIndex    int
	Command    string
	ArgKey     string
	ArgValue   string
	ShiftIndex int
	ShiftCmd   string
}

func (v HandleSafetyViolation) Error() string {
	return fmt.Sprintf("op %d (%s) targets an address-positional XLSX handle (%s=%q) after op %d (%s) shifted rows/columns earlier in the same batch; the handle's A1 address may move, risking a silent wrong-cell write. Run the structural edit and the handle op as separate apply invocations (re-resolving the handle against the post-edit file), or target the cell positionally with --sheet/--cell.",
		v.OpIndex, v.Command, v.ArgKey, v.ArgValue, v.ShiftIndex, v.ShiftCmd)
}

// FirstHandleSafetyViolation rejects a batch in which a structural row/column
// shift precedes an op targeting an address-positional XLSX handle.
func FirstHandleSafetyViolation(ops []Operation) (HandleSafetyViolation, bool) {
	shiftedAt := -1
	shiftCmd := ""
	for i, op := range ops {
		norm := NormalizeCommand(op.Command)
		if StructuralShiftCommand(norm) {
			if shiftedAt < 0 {
				shiftedAt = i
				shiftCmd = norm
			}
			continue
		}
		if shiftedAt < 0 {
			continue
		}
		if key, value, ok := FirstAddressPositionalArg(op); ok {
			return HandleSafetyViolation{
				OpIndex:    i,
				Command:    norm,
				ArgKey:     key,
				ArgValue:   value,
				ShiftIndex: shiftedAt,
				ShiftCmd:   shiftCmd,
			}, true
		}
	}
	return HandleSafetyViolation{}, false
}
