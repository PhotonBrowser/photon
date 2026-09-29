import { PhotonWebView } from "./components/PhotonWebView";

export function App() {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 8,
        backgroundColor: "#111111",
      }}
    >
      <PhotonWebView
        url="https://example.com"
        style={{
          width: "100%",
          height: "100%",
          borderRadius: 12,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
