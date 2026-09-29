import { PhotonWebView } from "./components/PhotonWebView";

export function App() {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 4,
        backgroundColor: "#111111",
        borderRadius: 12,
      }}
    >
      <PhotonWebView
        url="https://example.com"
        style={{
          width: "100%",
          height: "100%",
          borderRadius: 6,
          overflow: "hidden",
        }}
      />
    </div>
  );
}
