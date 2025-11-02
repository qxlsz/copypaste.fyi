import { test, expect } from '@playwright/test'

test.describe('Webpage Health Check', () => {
  test('homepage loads without white screen', async ({ page }) => {
    // Navigate to the homepage
    await page.goto('http://localhost:5173')

    // Wait for the page to load
    await page.waitForLoadState('domcontentloaded')

    // Check that the body is visible (not white/empty)
    const body = page.locator('body')
    await expect(body).toBeVisible()

    // Check that there's actual content (not just empty)
    const bodyText = await body.textContent()
    expect(bodyText?.length).toBeGreaterThan(10)
  })

  test('header and navigation render correctly', async ({ page }) => {
    await page.goto('http://localhost:5173')

    // Check that the main branding is present
    await expect(page.getByText('copypaste.fyi')).toBeVisible()

    // Check that navigation links are present
    await expect(page.getByText('Create Paste')).toBeVisible()
    await expect(page.getByText('Dashboard')).toBeVisible()
    await expect(page.getByText('Stats')).toBeVisible()
  })

  test('paste form elements are present', async ({ page }) => {
    await page.goto('http://localhost:5173')

    // Check that the main form heading is present
    await expect(page.getByText('Create a secure paste')).toBeVisible()

    // Check that form inputs exist
    await expect(page.getByPlaceholder('Paste or type your content here...')).toBeVisible()

    // Check that the submit button is present
    await expect(page.getByText('Create paste')).toBeVisible()
  })

  test('shows key prompt for encrypted paste', async ({ page }) => {
    // This test would require mocking the API to return a 401 error
    // For now, we'll just verify the UI structure exists
    await page.goto('http://localhost:5173')
    
    // The key prompt form should exist in the component but not be visible initially
    // This test ensures the form elements are present in the DOM
    const keyInput = page.locator('input[type="password"]').first()
    await expect(keyInput).toBeAttached() // Element exists but may not be visible
  })
})
