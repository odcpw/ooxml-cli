Attribute VB_Name = "AgentSmoke"
Public Sub AgentSmokeRun()
    Dim worker As Worker
    Set worker = New Worker
    ThisWorkbook.Worksheets(1).Range("A1").Value = worker.Message()
End Sub
