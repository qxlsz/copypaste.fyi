# Sign in with Google

Get link stays anonymous. Google is optional, for sharing.

## Share with Gmail

The share row has Gmail. It opens compose with the paste URL. No key in that URL.

## Sign in

Set a Web client ID from Google Cloud (OAuth 2.0, authorized JS origins = your site):

```bash
export COPYPASTE_GOOGLE_CLIENT_ID='....apps.googleusercontent.com'
```

`GET /api/auth/providers` then returns `{ "google": "<id>" }`. The share row Sign in with Google button uses Google Identity Services, then `POST /api/auth/google` with the ID token. The server checks the token at Google and opens a session.

Public copypaste.fyi can leave this unset. Self-hosters turn it on when they want named shares.
