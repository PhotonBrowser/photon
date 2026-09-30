import type { Theme } from "../theme";

interface WorkspaceProps {
  theme: Theme;
}

export function Workspace({ theme }: WorkspaceProps) {
  return <div style={{ flexGrow: 1, backgroundColor: theme.color.window }} />;
}
