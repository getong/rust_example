import { memo, useState } from "react";
import type { JSX } from "react";

const MyComponent = memo(
  (props) => {
    console.log("Rendering MyComponent");
    return <div>{props.text}</div>;
  },
  (prevProps, nextProps) => prevProps.text === nextProps.text,
);

export default function App(): JSX.Element {
  const [count, setCount] = useState(0);
  const [text, setText] = useState("Hello, world!");

  return (
    <div>
      <h2>Current Count: {count}</h2>
      <h2>Current Text: {text}</h2>
      <button
        onClick={() => {
          console.log("Count before:", count);
          setCount(count + 1); // State update is scheduled
          console.log("Count after:", count + 1); // Shows outdated value because state hasn't updated yet
        }}
      >
        Increment Count
      </button>

      <button
        onClick={() => {
          console.log("Text before:", text);
          setText(`Text updated! ${Date.now()}`);
          console.log("Text after:", text); // Similar issue as above
        }}
      >
        Change Text
      </button>
      <MyComponent text={text} />
    </div>
  );
}
