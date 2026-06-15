package model

// StyleInfo describes a single style definition from word/styles.xml.
type StyleInfo struct {
	StyleID         string   `json:"styleId"`
	Name            string   `json:"name,omitempty"`
	Type            string   `json:"type,omitempty"`
	Default         bool     `json:"default"`
	Builtin         bool     `json:"builtin"`
	BasedOn         string   `json:"basedOn,omitempty"`
	Next            string   `json:"next,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	// Handle is the stable style handle (H:docx/pt:styles/style:n:<styleId>)
	// built from the native w:styleId. Same string the mutate side accepts.
	Handle string `json:"handle,omitempty"`
}
