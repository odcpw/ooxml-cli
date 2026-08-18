VERSION 5.00
Begin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} Dialog
   Caption = "Golden Dialog"
End
Attribute VB_Name = "Dialog"
Attribute VB_Base = "0{F8A47041-B2A6-11CE-8027-00AA00611080}"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = True
Attribute VB_Exposed = False
Attribute VB_TemplateDerived = False
Attribute VB_Customizable = False
Private Sub UserForm_Initialize()
    Caption = "Runtime caption must not replace designer caption"
End Sub
