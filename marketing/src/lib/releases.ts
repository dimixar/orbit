const LATEST_RELEASE_API =
  "https://api.github.com/repos/imrj05/orbit/releases/latest";
const RELEASES_PAGE = "https://github.com/imrj05/orbit/releases";

/** Where to send someone when we cannot resolve a concrete asset. */
export const LATEST_RELEASE_URL = `${RELEASES_PAGE}/latest`;

export type Release = {
  tag: string;
  version: string;
  name: string;
  pageUrl: string;
  publishedAt: string;
  macos?: string;
  windows?: string;
  linux?: string;
};

type GithubAsset = { name?: string; browser_download_url?: string };

function assetUrl(assets: GithubAsset[], pattern: RegExp) {
  return assets.find((a) => a.name && pattern.test(a.name))?.browser_download_url;
}

/**
 * Latest published (non-prerelease) GitHub release, cached for an hour.
 * Returns `null` when the API is unavailable so callers can fall back to the
 * releases page instead of shipping a stale hard-coded version.
 */
export async function getLatestRelease(): Promise<Release | null> {
  try {
    const res = await fetch(LATEST_RELEASE_API, {
      next: { revalidate: 3600 },
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return null;

    const data = (await res.json()) as {
      tag_name?: string;
      name?: string;
      html_url?: string;
      published_at?: string;
      assets?: GithubAsset[];
    };

    const tag = data.tag_name ?? "";
    if (!tag) return null;

    const assets = Array.isArray(data.assets) ? data.assets : [];
    return {
      tag,
      version: tag.replace(/^v/, ""),
      name: data.name || `Orbit ${tag}`,
      pageUrl: data.html_url || `${RELEASES_PAGE}/tag/${tag}`,
      publishedAt: data.published_at ?? "",
      macos: assetUrl(assets, /\.dmg$/i),
      windows: assetUrl(assets, /windows.*\.zip$/i) ?? assetUrl(assets, /\.zip$/i),
      linux:
        assetUrl(assets, /linux.*\.tar\.gz$/i) ??
        assetUrl(assets, /\.tar\.gz$/i),
    };
  } catch {
    return null;
  }
}
