Attribute VB_Name = "AgentDoc"
Public Sub MarkDocument()
    Dim worker As Worker
    Set worker = New Worker
    ActiveDocument.Range.InsertAfter worker.Message()
End Sub
