package testutil

import (
	"flamingo.run/openframe-cli/internal/cluster/models"
)

// TestClusterConfig creates a test cluster configuration
func TestClusterConfig(name string) *models.ClusterConfig {
	return &models.ClusterConfig{
		Name:       name,
		Type:       models.ClusterTypeK3d,
		NodeCount:  3,
		K8sVersion: "v1.28.0",
	}
}
