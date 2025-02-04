import React from "react";
import ReactDOM from "react-dom";

const MyComponent = React.memo(
  (props) => {
    console.log("Rendering MyComponent");
    return <div>{props.text}</div>;
  },
  (prevProps, nextProps) => prevProps.text === nextProps.text,
);

export default function App() {
  const [count, setCount] = React.useState(0);
  const [text, setText] = React.useState("Hello, world!");

  return (
    <div>
      <h2>Current Count: {count}</h2>
      <h2>Current Text: {text}</h2>
      <button onClick={() => setCount(count + 1)}>Increment Count</button>
      <button onClick={() => setText(`Text updated! ${Date.now()}`)}>
        Change Text
      </button>
      <MyComponent text={text} />
    </div>
  );
}
