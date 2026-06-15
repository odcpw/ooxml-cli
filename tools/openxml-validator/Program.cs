using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

class P {
  sealed class ValidationIssue {
    public string ErrorType { get; set; } = "";
    public string Description { get; set; } = "";
    public string Part { get; set; } = "";
    public string Node { get; set; } = "";
    public string XPath { get; set; } = "";
  }

  sealed class ValidationSummary {
    public string File { get; set; } = "";
    public bool Valid { get; set; }
    public int ErrorCount { get; set; }
    public string Schema { get; set; } = "Office2019";
    public List<ValidationIssue> Errors { get; set; } = new List<ValidationIssue>();
  }

  static List<ValidationIssue> Run(OpenXmlValidator v, OpenXmlPackage d) =>
    v.Validate(d).Select(e => new ValidationIssue {
      ErrorType = e.ErrorType.ToString(),
      Description = e.Description ?? "",
      Part = e.Part?.Uri?.ToString() ?? "",
      Node = e.Node?.LocalName ?? "",
      XPath = e.Path?.XPath ?? "",
    }).ToList();

  static List<ValidationIssue> ValidatePath(OpenXmlValidator v, string path) {
    var ext = Path.GetExtension(path).ToLowerInvariant();
    switch (ext) {
      case ".pptx":
      case ".pptm":
        using (var d = PresentationDocument.Open(path, false)) return Run(v, d);
      case ".docx":
      case ".docm":
        using (var d = WordprocessingDocument.Open(path, false)) return Run(v, d);
      case ".xlsx":
      case ".xlsm":
        using (var d = SpreadsheetDocument.Open(path, false)) return Run(v, d);
      default:
        return new List<ValidationIssue> {
          new ValidationIssue {
            ErrorType = "UnsupportedExtension",
            Description = "Supported extensions: .docx, .docm, .pptx, .pptm, .xlsx, .xlsm.",
          },
        };
    }
  }

  static int Main(string[] args) {
    var json = args.Contains("--json");
    var path = args.FirstOrDefault(a => !a.StartsWith("--", StringComparison.Ordinal));
    if (string.IsNullOrEmpty(path)) {
      Console.Error.WriteLine("usage: openxml-validator [--json] <file>");
      return 2;
    }

    var v = new OpenXmlValidator(FileFormatVersions.Office2019);
    List<ValidationIssue> errs;
    try {
      errs = ValidatePath(v, path);
    }
    catch (Exception ex) {
      errs = new List<ValidationIssue> {
        new ValidationIssue {
          ErrorType = ex.GetType().FullName ?? "Exception",
          Description = ex.Message,
        },
      };
    }

    var summary = new ValidationSummary {
      File = Path.GetFullPath(path),
      Valid = errs.Count == 0,
      ErrorCount = errs.Count,
      Errors = errs,
    };

    if (json) {
      Console.WriteLine(JsonSerializer.Serialize(summary, new JsonSerializerOptions { WriteIndented = true }));
    }
    else {
      foreach (var e in errs) {
        Console.WriteLine($"[{e.ErrorType}] {e.Description}");
        Console.WriteLine($"   part={e.Part} node=<{e.Node}> xpath={e.XPath}");
      }
      Console.WriteLine(errs.Count == 0 ? "OPENXML-VALIDATOR: 0 errors (clean)" : $"OPENXML-VALIDATOR: {errs.Count} error(s)");
    }

    return errs.Count == 0 ? 0 : 1;
  }
}
