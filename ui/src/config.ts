export const DEFAULT_URL = "https://example.com";

export function getStartupUrl() {
  return process.env.PHOTON_URL ?? DEFAULT_URL;
}
