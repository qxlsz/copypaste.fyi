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

  test('no JavaScript errors in console', async ({ page }) => {
    const errors: string[] = []

    page.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text())
      }
    })

    await page.goto('http://localhost:5173')
    await page.waitForLoadState('networkidle')

    // Filter out common non-critical errors
    const criticalErrors = errors.filter(error =>
      !error.includes('Download the React DevTools') &&
      !error.includes('devtools') &&
      !error.includes('favicon')
    )

    expect(criticalErrors).toHaveLength(0)
  })
})
