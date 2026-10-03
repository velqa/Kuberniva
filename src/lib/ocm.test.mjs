import assert from 'node:assert/strict';
import { test } from 'node:test';
import { clusterSet, managedCluster, manifestWork, placement, policy } from './ocm.ts';

const cluster = (name, available, extra = {}) => managedCluster({
  metadata: { name, labels: { 'cluster.open-cluster-management.io/clusterset': 'prod', region: 'us-east-1' } },
  spec: { hubAcceptsClient: true, managedClusterClientConfigs: [{ url: `https://${name}.example.com:6443` }], taints: extra.taints },
  status: {
    conditions: [{ type: 'HubAcceptedManagedCluster', status: 'True' }, { type: 'ManagedClusterJoined', status: 'True' }, { type: 'ManagedClusterConditionAvailable', status: available }],
    version: { kubernetes: 'v1.30.4' }, allocatable: { cpu: '15800m', memory: '61Gi' },
    clusterClaims: [{ name: 'platform.open-cluster-management.io', value: 'AWS' }],
  },
});

test('managed clusters report health, set membership, capacity, and claims', () => {
  const ok = cluster('east', 'True', { taints: [{ key: 'maintenance', effect: 'NoSelect' }] });
  assert.equal(ok.health, 'Healthy');
  assert.equal(ok.clusterSet, 'prod');
  assert.equal(ok.kubernetes, 'v1.30.4');
  assert.equal(ok.cpu, '15.8 cores');
  assert.equal(ok.memory, '61Gi');
  assert.deepEqual(ok.claims[0], { name: 'platform.open-cluster-management.io', value: 'AWS' });
  assert.deepEqual(ok.taints, ['maintenance:NoSelect']);
  assert.equal(cluster('west', 'False').health, 'Degraded');
  assert.equal(cluster('lost', 'Unknown').health, 'Unknown');
  assert.equal(managedCluster({ metadata: { name: 'new' }, spec: { hubAcceptsClient: false } }).health, 'Pending');
  assert.equal(clusterSet({ metadata: { name: 'prod' } }, [ok, cluster('west', 'False')]).members, 2);
});

test('placements, manifest works, and policies summarise their status', () => {
  const decisions = [{ metadata: { namespace: 'apps', labels: { 'cluster.open-cluster-management.io/placement': 'web' } }, status: { decisions: [{ clusterName: 'west' }, { clusterName: 'east' }] } }];
  const web = placement({ metadata: { name: 'web', namespace: 'apps' }, spec: { clusterSets: ['prod'], numberOfClusters: 2 }, status: { numberOfSelectedClusters: 2, conditions: [{ type: 'PlacementSatisfied', status: 'True' }] } }, decisions);
  assert.deepEqual([web.requested, web.selected, web.satisfied, web.decisions], ['2', 2, 'True', ['east', 'west']]);
  const work = manifestWork({
    metadata: { name: 'web-app', namespace: 'east' }, spec: { workload: { manifests: [{}, {}] } },
    status: { conditions: [{ type: 'Applied', status: 'True' }, { type: 'Available', status: 'True' }], resourceStatus: { manifests: [{ resourceMeta: { kind: 'Deployment', name: 'web', namespace: 'apps' }, conditions: [{ type: 'Available', status: 'False' }] }] } },
  });
  assert.equal(work.cluster, 'east');
  assert.equal(work.resources, 2);
  assert.equal(work.health, 'Degraded');
  assert.deepEqual(work.failing, ['Deployment apps/web']);
  const pol = policy({ metadata: { name: 'require-labels', namespace: 'policies' }, spec: { remediationAction: 'enforce' }, status: { compliant: 'NonCompliant', status: [{ clustername: 'east', compliant: 'Compliant' }, { clustername: 'west', compliant: 'NonCompliant' }] } });
  assert.equal(pol.remediation, 'enforce');
  assert.deepEqual(pol.clusters.map((entry) => entry.compliant), ['Compliant', 'NonCompliant']);
});
