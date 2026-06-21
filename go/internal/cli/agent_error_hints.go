package cli

import (
	"fmt"
	"sort"
	"strings"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

const unknownCommandProbe = "__ooxml_unknown_command_probe__"

// installAgentErrorHints upgrades Cobra's generic unknown command/flag errors
// into copy-pasteable guidance. It runs after the full command tree is assembled
// so suggestions can use the real subcommand and flag inventory.
func installAgentErrorHints(root *cobra.Command) {
	if root == nil {
		return
	}
	root.SetFlagErrorFunc(flagErrorWithHints)
	walkCommands(root, func(cmd *cobra.Command) {
		cmd.SuggestionsMinimumDistance = 2
		if cmd.HasSubCommands() && argsLooksLikeNoArgs(cmd) {
			cmd.Args = noSubcommandArgsWithHints
		}
	})
}

func walkCommands(cmd *cobra.Command, fn func(*cobra.Command)) {
	if cmd == nil {
		return
	}
	fn(cmd)
	for _, child := range cmd.Commands() {
		walkCommands(child, fn)
	}
}

func argsLooksLikeNoArgs(cmd *cobra.Command) bool {
	if cmd == nil || cmd.Args == nil {
		return false
	}
	err := cmd.Args(cmd, []string{unknownCommandProbe})
	if err == nil {
		return false
	}
	want := fmt.Sprintf("unknown command %q for %q", unknownCommandProbe, cmd.CommandPath())
	return strings.HasPrefix(err.Error(), want)
}

func noSubcommandArgsWithHints(cmd *cobra.Command, args []string) error {
	if len(args) == 0 {
		return nil
	}
	unknown := args[0]
	message := fmt.Sprintf("unknown command %q for %q", unknown, cmd.CommandPath())
	suggestions := cmd.SuggestionsFor(unknown)
	if len(suggestions) > 0 {
		message += "; did you mean: " + strings.Join(suggestions, ", ")
		tryArgs := append(strings.Fields(cmd.CommandPath()), suggestions[0])
		tryArgs = append(tryArgs, args[1:]...)
		message += "; try: `" + shellCommandFromArgs(tryArgs...) + "`"
	}
	message += "; discover with `" + shellCommandFromArgs(append(strings.Fields(cmd.CommandPath()), "--help")...) + "`"
	return InvalidArgsError(message)
}

func flagErrorWithHints(cmd *cobra.Command, err error) error {
	if err == nil {
		return nil
	}
	message := err.Error()
	const prefix = "unknown flag: "
	if strings.HasPrefix(message, prefix) {
		unknown := strings.TrimSpace(strings.TrimPrefix(message, prefix))
		suggestions := flagSuggestions(cmd, unknown, maxSelectorCandidates)
		if len(suggestions) > 0 {
			message += "; did you mean: " + strings.Join(suggestions, ", ")
			message += "; retry with `" + suggestions[0] + "`"
		}
		if cmd != nil {
			message += "; discover with `" + shellCommandFromArgs(append(strings.Fields(cmd.CommandPath()), "--help")...) + "`"
		}
	}
	return InvalidArgsError(message)
}

func flagSuggestions(cmd *cobra.Command, unknown string, max int) []string {
	if cmd == nil {
		return nil
	}
	needle := strings.TrimLeft(strings.TrimSpace(unknown), "-")
	if needle == "" {
		return nil
	}
	if max <= 0 {
		max = maxSelectorCandidates
	}

	candidates := knownFlagNames(cmd)
	type scoredFlag struct {
		name  string
		score int
	}
	scored := make([]scoredFlag, 0, len(candidates))
	for _, name := range candidates {
		score := agentHintEditDistance(strings.ToLower(needle), strings.ToLower(strings.TrimLeft(name, "-")))
		if score <= 2 || strings.HasPrefix(strings.ToLower(strings.TrimLeft(name, "-")), strings.ToLower(needle)) {
			scored = append(scored, scoredFlag{name: name, score: score})
		}
	}
	sort.SliceStable(scored, func(i, j int) bool {
		if scored[i].score != scored[j].score {
			return scored[i].score < scored[j].score
		}
		return scored[i].name < scored[j].name
	})

	out := make([]string, 0, max)
	for _, item := range scored {
		out = append(out, item.name)
		if len(out) >= max {
			break
		}
	}
	return out
}

func knownFlagNames(cmd *cobra.Command) []string {
	seen := map[string]bool{}
	var out []string
	addSet := func(flags *pflag.FlagSet) {
		if flags == nil {
			return
		}
		flags.VisitAll(func(flag *pflag.Flag) {
			if flag == nil || flag.Hidden {
				return
			}
			long := "--" + flag.Name
			if !seen[long] {
				seen[long] = true
				out = append(out, long)
			}
			if flag.Shorthand != "" {
				short := "-" + flag.Shorthand
				if !seen[short] {
					seen[short] = true
					out = append(out, short)
				}
			}
		})
	}
	addSet(cmd.Flags())
	addSet(cmd.InheritedFlags())
	addSet(cmd.PersistentFlags())
	sort.Strings(out)
	return out
}

func shellCommandFromArgs(args ...string) string {
	quoted := make([]string, 0, len(args))
	for _, arg := range args {
		quoted = append(quoted, pptxXLSXCommandArg(arg))
	}
	return strings.Join(quoted, " ")
}

func agentHintEditDistance(a, b string) int {
	ar := []rune(a)
	br := []rune(b)
	if len(ar) == 0 {
		return len(br)
	}
	if len(br) == 0 {
		return len(ar)
	}

	prev := make([]int, len(br)+1)
	cur := make([]int, len(br)+1)
	for j := range prev {
		prev[j] = j
	}
	for i, ra := range ar {
		cur[0] = i + 1
		for j, rb := range br {
			cost := 0
			if ra != rb {
				cost = 1
			}
			cur[j+1] = minInt3(cur[j]+1, prev[j+1]+1, prev[j]+cost)
		}
		prev, cur = cur, prev
	}
	return prev[len(br)]
}

func minInt3(a, b, c int) int {
	if b < a {
		a = b
	}
	if c < a {
		a = c
	}
	return a
}
