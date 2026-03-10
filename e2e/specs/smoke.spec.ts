describe("smoke", () => {
  it("should launch and render the app", async () => {
    // The app should have a title
    const title = await browser.getTitle();
    expect(title).toBe("The Controller");
  });

  it("should render the sidebar", async () => {
    // The sidebar has the project tree
    const sidebar = await $(".sidebar");
    await sidebar.waitForExist({ timeout: 10_000 });
    expect(await sidebar.isDisplayed()).toBe(true);
  });
});
