package cli

import (
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func openPackageExpectType(path string, expected opc.PackageType) (*opc.Package, error) {
	pkg, err := opc.Open(path)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
	}

	detected := opc.DetectType(pkg)
	if detected != expected {
		_ = pkg.Close()
		return nil, NewCLIErrorf(
			ExitUnsupportedType,
			"file is not a %s document (detected: %s)",
			strings.ToUpper(expected.String()),
			detected,
		)
	}

	return pkg, nil
}
