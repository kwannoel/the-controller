import { describe, it, expect } from "vitest";
import { get } from "svelte/store";
import {
  deployedServices,
  selectedServiceId,
  serviceLogLines,
} from "./deploy-stores";

describe("deploy stores", () => {
  it("starts with empty services list", () => {
    expect(get(deployedServices)).toEqual([]);
  });

  it("starts with no selected service", () => {
    expect(get(selectedServiceId)).toBeNull();
  });

  it("starts with empty log lines", () => {
    expect(get(serviceLogLines)).toEqual([]);
  });
});
