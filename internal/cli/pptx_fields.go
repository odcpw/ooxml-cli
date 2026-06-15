package cli

import "github.com/spf13/cobra"

var pptxFieldsCmd = &cobra.Command{
	Use:   "fields",
	Short: "Inspect and set header/footer/slide-number/date fields",
	Long: `Inspect and set presentation header/footer/slide-number/date fields.

PowerPoint stores these fields in two places: presentation-wide visibility
toggles live on each slide master's p:hf element, while the actual footer text,
date, and slide number are rendered by placeholder shapes (a:ph type="ftr"/"dt"/
"sldNum") on individual slides. There is no hdrFtr element in presentation.xml.

  inspect  report master visibility defaults and per-slide field placeholders
  set      toggle visibility on masters and set footer text / date format on slides`,
}

func init() {
	pptxCmd.AddCommand(pptxFieldsCmd)
}
