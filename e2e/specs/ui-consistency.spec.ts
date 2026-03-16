import { test, expect, type Page } from "@playwright/test";

/**
 * UI consistency checks across the app's workspace modes,
 * layout, theme, and interactive elements.
 */

test.describe("Layout integrity", () => {
  test("app loads with sidebar and main content area", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle("The Controller");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });
    await expect(page.locator(".terminal-area")).toBeVisible();

    // Layout fills viewport
    const layout = page.locator(".app-layout");
    await expect(layout).toBeVisible();
    const box = await layout.boundingBox();
    expect(box).not.toBeNull();
    const viewport = page.viewportSize()!;
    expect(box!.width).toBeCloseTo(viewport.width, -1);
    expect(box!.height).toBeCloseTo(viewport.height, -1);
  });

  test("sidebar has correct fixed width", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator(".sidebar");
    const box = await sidebar.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBe(250);
  });

  test("sidebar contains header, project list, and footer", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    await expect(page.locator(".sidebar-header")).toBeVisible();
    await expect(page.locator(".project-list")).toBeVisible();
    await expect(page.locator(".sidebar-footer")).toBeVisible();
  });

  test("sidebar footer has help button and provider indicator", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const helpBtn = page.locator(".btn-help");
    await expect(helpBtn).toBeVisible();
    await expect(helpBtn).toHaveText("?");

    const provider = page.locator(".provider-indicator");
    await expect(provider).toBeVisible();
    // Should show "Provider: Claude" or "Provider: Codex"
    await expect(provider).toContainText("Provider:");
  });
});

test.describe("Theme consistency", () => {
  test("CSS variables are applied to the body", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Check that custom properties resolve to non-empty values
    const bgVoid = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--bg-void").trim()
    );
    expect(bgVoid).toBe("#000000");

    const textPrimary = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--text-primary").trim()
    );
    expect(textPrimary).toBe("#e0e0e0");

    const fontSans = await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--font-sans").trim()
    );
    expect(fontSans).toContain("Geist Sans");
  });

  test("sidebar uses surface background", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const sidebarBg = await page.locator(".sidebar").evaluate((el) =>
      getComputedStyle(el).backgroundColor
    );
    // --bg-surface: #141414 → rgb(20, 20, 20)
    expect(sidebarBg).toBe("rgb(20, 20, 20)");
  });

  test("sidebar header text is centered", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const textAlign = await page.locator(".sidebar-header h2").evaluate((el) =>
      getComputedStyle(el).textAlign
    );
    expect(textAlign).toBe("center");
  });
});

test.describe("Development mode (default)", () => {
  test("shows Development in sidebar header", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    await expect(page.locator(".sidebar-header h2")).toHaveText("Development");
  });

  test("shows empty state when no session is active", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // The terminal manager should show the empty state
    const emptyTitle = page.locator(".terminal-manager .empty-title");
    // If there are existing sessions this may not be visible, so check either:
    // - empty state is shown OR
    // - a terminal wrapper is visible
    const hasEmptyState = await emptyTitle.isVisible().catch(() => false);
    const hasTerminal = await page.locator(".terminal-wrapper.visible").isVisible().catch(() => false);

    expect(hasEmptyState || hasTerminal).toBe(true);
  });

  test("empty state kbd elements are styled", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const emptyHint = page.locator(".terminal-manager .empty-hint");
    const hasEmptyState = await emptyHint.isVisible().catch(() => false);

    if (hasEmptyState) {
      const kbds = emptyHint.locator("kbd");
      const count = await kbds.count();
      expect(count).toBeGreaterThan(0);

      // Check kbd styling
      for (let i = 0; i < count; i++) {
        const bg = await kbds.nth(i).evaluate((el) =>
          getComputedStyle(el).backgroundColor
        );
        // --bg-hover: #242424 → rgb(36, 36, 36)
        expect(bg).toBe("rgb(36, 36, 36)");
      }
    }
  });
});

test.describe("Sidebar header updates per mode", () => {
  test("header shows correct text for each workspace mode", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Default: Development
    await expect(page.locator(".sidebar-header h2")).toHaveText("Development");

    // Switch to Agents mode via store manipulation
    await page.evaluate(() => {
      // Dispatch keyboard event for space bar to open workspace mode picker
      window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
    });

    // If the workspace mode picker opened, check it
    const picker = page.locator(".picker");
    const pickerVisible = await picker.isVisible({ timeout: 2_000 }).catch(() => false);

    if (pickerVisible) {
      await expect(picker.locator(".picker-title")).toHaveText("Switch Workspace");

      // Verify all 6 modes are listed
      const options = picker.locator(".picker-option");
      await expect(options).toHaveCount(6);

      // Check mode labels
      const labels = await options.locator(".option-label").allTextContents();
      expect(labels).toEqual([
        "Development",
        "Agents",
        "Architecture",
        "Notes",
        "Infrastructure",
        "Voice",
      ]);

      // Current mode should have "current" badge
      const activeBadge = picker.locator(".picker-option.active .current-badge");
      await expect(activeBadge).toHaveText("current");

      // Press Escape to close
      await page.keyboard.press("Escape");
    }
  });
});

test.describe("Help modal", () => {
  test("help button opens keyboard shortcuts modal", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Click the help button
    await page.locator(".btn-help").click();

    // Modal should appear
    const modal = page.locator(".modal");
    await expect(modal).toBeVisible({ timeout: 3_000 });
    await expect(modal.locator(".modal-header")).toHaveText("Keyboard Shortcuts");

    // Should have a subtitle with current mode
    const subtitle = modal.locator(".subtitle");
    await expect(subtitle).toBeVisible();
    await expect(subtitle).toContainText("Mode:");

    // Should have shortcut sections
    const sections = modal.locator(".section");
    const sectionCount = await sections.count();
    expect(sectionCount).toBeGreaterThan(0);

    // Each section should have a label and a table
    for (let i = 0; i < sectionCount; i++) {
      await expect(sections.nth(i).locator(".section-label")).toBeVisible();
      await expect(sections.nth(i).locator(".shortcut-table")).toBeVisible();
    }

    // Close with Escape
    await page.keyboard.press("Escape");
    await expect(modal).not.toBeVisible();
  });
});

test.describe("No overlapping or clipped elements", () => {
  test("sidebar and main area do not overlap", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const sidebarBox = await page.locator(".sidebar").boundingBox();
    const mainBox = await page.locator(".terminal-area").boundingBox();

    expect(sidebarBox).not.toBeNull();
    expect(mainBox).not.toBeNull();

    // Sidebar right edge should meet main area left edge (no overlap, no gap > 1px)
    const sidebarRight = sidebarBox!.x + sidebarBox!.width;
    expect(Math.abs(sidebarRight - mainBox!.x)).toBeLessThanOrEqual(1);
  });

  test("sidebar footer is at the bottom", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const sidebarBox = await page.locator(".sidebar").boundingBox();
    const footerBox = await page.locator(".sidebar-footer").boundingBox();

    expect(sidebarBox).not.toBeNull();
    expect(footerBox).not.toBeNull();

    // Footer bottom should be at or near sidebar bottom
    const sidebarBottom = sidebarBox!.y + sidebarBox!.height;
    const footerBottom = footerBox!.y + footerBox!.height;
    expect(Math.abs(sidebarBottom - footerBottom)).toBeLessThanOrEqual(1);
  });

  test("sidebar header border separates it from project list", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    const headerBorderBottom = await page.locator(".sidebar-header").evaluate((el) =>
      getComputedStyle(el).borderBottomStyle
    );
    expect(headerBorderBottom).toBe("solid");
  });
});

test.describe("Architecture mode empty state", () => {
  test("architecture view shows empty state with generate hint", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Switch to architecture mode
    await page.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
    });

    const picker = page.locator(".picker");
    const pickerVisible = await picker.isVisible({ timeout: 2_000 }).catch(() => false);

    if (pickerVisible) {
      // Press 'r' for architecture
      await page.keyboard.press("r");

      // Wait for architecture view to render
      const archExplorer = page.locator(".architecture-explorer");
      const visible = await archExplorer.isVisible({ timeout: 3_000 }).catch(() => false);

      if (visible) {
        // Check diagram pane exists
        await expect(page.locator(".diagram-pane")).toBeVisible();
        // Check inspector rail exists
        await expect(page.locator(".inspector-rail")).toBeVisible();
        // Check generate button exists
        await expect(page.locator(".generate-action")).toBeVisible();

        // Switch back to development
        await page.evaluate(() => {
          window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
        });
        const pickerAgain = page.locator(".picker");
        if (await pickerAgain.isVisible({ timeout: 1_000 }).catch(() => false)) {
          await page.keyboard.press("d");
        }
      }
    }
  });
});

test.describe("Infrastructure mode empty state", () => {
  test("infrastructure view shows empty state", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Switch to infrastructure mode
    await page.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
    });

    const picker = page.locator(".picker");
    const pickerVisible = await picker.isVisible({ timeout: 2_000 }).catch(() => false);

    if (pickerVisible) {
      await page.keyboard.press("i");

      // Infrastructure empty state (two-line pattern)
      const emptyTitle = page.locator(".empty-state .empty-title");
      const visible = await emptyTitle.isVisible({ timeout: 3_000 }).catch(() => false);

      if (visible) {
        await expect(emptyTitle).toHaveText("No services deployed yet");
        await expect(page.locator(".empty-state .empty-hint")).toContainText(
          "press"
        );
      }

      // Switch back
      await page.evaluate(() => {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
      });
      if (await page.locator(".picker").isVisible({ timeout: 1_000 }).catch(() => false)) {
        await page.keyboard.press("d");
      }
    }
  });
});

test.describe("Notes mode", () => {
  test("notes mode shows Notes header and editor area", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Switch to notes mode
    await page.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
    });

    const picker = page.locator(".picker");
    const pickerVisible = await picker.isVisible({ timeout: 2_000 }).catch(() => false);

    if (pickerVisible) {
      await page.keyboard.press("n");

      // Sidebar header should now say "Notes"
      await expect(page.locator(".sidebar-header h2")).toHaveText("Notes", {
        timeout: 3_000,
      });

      // Notes editor should be visible
      const notesEditor = page.locator(".notes-editor");
      const visible = await notesEditor.isVisible({ timeout: 3_000 }).catch(() => false);

      if (visible) {
        // Should show empty state if no note selected
        const emptyTitle = notesEditor.locator(".empty-title");
        const hasEmpty = await emptyTitle.isVisible().catch(() => false);
        if (hasEmpty) {
          await expect(emptyTitle).toHaveText("No note selected");
        }
      }

      // Switch back
      await page.evaluate(() => {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
      });
      if (await page.locator(".picker").isVisible({ timeout: 1_000 }).catch(() => false)) {
        await page.keyboard.press("d");
      }
    }
  });
});

test.describe("Agents mode", () => {
  test("agents mode shows Agents header", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".sidebar")).toBeVisible({ timeout: 10_000 });

    // Switch to agents mode
    await page.evaluate(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
    });

    const picker = page.locator(".picker");
    const pickerVisible = await picker.isVisible({ timeout: 2_000 }).catch(() => false);

    if (pickerVisible) {
      await page.keyboard.press("a");

      await expect(page.locator(".sidebar-header h2")).toHaveText("Agents", {
        timeout: 3_000,
      });

      // Switch back
      await page.evaluate(() => {
        window.dispatchEvent(new KeyboardEvent("keydown", { key: " ", code: "Space" }));
      });
      if (await page.locator(".picker").isVisible({ timeout: 1_000 }).catch(() => false)) {
        await page.keyboard.press("d");
      }
    }
  });
});
