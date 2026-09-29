import { useState } from "react";
import type { ReactNode } from "react";
import { theme } from "../theme";

interface ButtonProps {
  children: ReactNode;
  onClick: () => void;
  label: string;
  icon?: boolean;
  color?: string;
  pressedColor?: string;
  iconColor?: string;
}

export function Button({
  children,
  onClick,
  label,
  icon = false,
  color = theme.color.button,
  pressedColor = theme.color.buttonPressed,
  iconColor,
}: ButtonProps) {
  const [pressed, setPressed] = useState(false);
  const size = icon
    ? theme.layout.titlebarActionSize - theme.space.xxs * 2
    : undefined;
  return (
    // GPUIX exposes div as its native interactive host element.
    // biome-ignore lint/a11y/useSemanticElements: GPUIX has no native button element.
    <div
      role="button"
      tabIndex={0}
      aria-label={label}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === "enter" && !event.isHeld) onClick();
        if (event.key === "space") event.preventDefault();
      }}
      onKeyUp={(event) => {
        if (event.key === "space" && !event.defaultPrevented) onClick();
      }}
      onMouseDown={() => setPressed(true)}
      onMouseUp={() => setPressed(false)}
      onMouseLeave={() => setPressed(false)}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: icon ? theme.space.xxs : theme.space.sm,
        borderRadius: icon ? theme.radius.md : theme.radius.sm,
        backgroundColor: pressed ? pressedColor : color,
        cursor: "pointer",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: size,
          height: size,
          opacity: pressed ? 0.9 : 1,
          color: iconColor,
        }}
      >
        {children}
      </div>
    </div>
  );
}

export function IconButton(props: ButtonProps) {
  return <Button {...props} icon />;
}
