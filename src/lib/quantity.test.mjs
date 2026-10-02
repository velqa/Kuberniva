import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cpuLabel, memoryLabel, percentLabel, resourceQuantityLabel } from './quantity.ts';

test('CPU quantities round to a readable number of cores', () => {
  assert.equal(cpuLabel('2236230912n'), '2.24 cores');
  assert.equal(cpuLabel('101234567n'), '0.1 cores');
  assert.equal(cpuLabel('250m'), '0.25 cores');
  assert.equal(cpuLabel('4'), '4 cores');
  assert.equal(cpuLabel('1'), '1 core');
  assert.equal(cpuLabel('48.123'), '48.1 cores');
  assert.equal(cpuLabel('5u'), '<0.001 cores');
  assert.equal(cpuLabel('0.042 cores'), '0.042 cores');
  assert.equal(cpuLabel(undefined), '—');
  assert.equal(cpuLabel('weird'), 'weird');
});

test('memory quantities use binary units with sensible precision', () => {
  assert.equal(memoryLabel('1846532Ki'), '1.8Gi');
  assert.equal(memoryLabel('16374240Ki'), '15.6Gi');
  assert.equal(memoryLabel('64Gi'), '64Gi');
  assert.equal(memoryLabel('184Mi'), '184Mi');
  assert.equal(memoryLabel('128974848'), '123Mi');
  assert.equal(memoryLabel('100G'), '93.1Gi');
});

test('percentages are whole numbers and tiny values stay visible', () => {
  assert.equal(percentLabel(2.2362309125), '2');
  assert.equal(percentLabel(0.4), '<1');
  assert.equal(percentLabel(0), '0');
  assert.equal(percentLabel(undefined), '—');
});

test('node resources format by name and keep counts unchanged', () => {
  assert.equal(resourceQuantityLabel('ephemeral-storage', '101430960Ki'), '96.7Gi');
  assert.equal(resourceQuantityLabel('pods', '110'), '110');
});
