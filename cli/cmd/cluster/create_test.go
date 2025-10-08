package cluster

import (
	"testing"

	"flamingo.run/openframe-cli/internal/cluster/utils"
	"flamingo.run/openframe-cli/tests/testutil"
)

func init() {
	testutil.InitializeTestMode()
}

func TestCreateCommand(t *testing.T) {
	setupFunc := func() {
		utils.SetTestExecutor(testutil.NewTestMockExecutor())
	}
	teardownFunc := func() {
		utils.ResetGlobalFlags()
	}

	testutil.TestClusterCommand(t, "create", getCreateCmd, setupFunc, teardownFunc)
}
