import React, { useEffect, useState } from 'react';
import { main_service } from './openapis';

const client = new main_service();

function App() {
  const [counter, setCounter] = useState(0);
  const [a, setA] = useState(0);
  const [b, setB] = useState(0);
  const [result, _setResult] = useState(0);
  const [error, setError] = useState('');

  useEffect(() => {
    client.counter.getCounter().then(v => setCounter(v.result));
  }, []);

  function setResult(result: number) {
    _setResult(result);
    setError('');
  }
  return (
    <div className="App">
      <header className="App-header">
        <h1>Counter: {counter}</h1>
      </header>
      <button onClick={() => client.counter.getCounter().then(v => setCounter(v.result))}>Update counter</button><br/>
      <label>
        A: <input type="number" value={a} onChange={(e) => setA(parseInt(e.target.value))}/>
      </label>
      <label>
        B: <input type="number" value={b} onChange={(e) => setB(parseInt(e.target.value))}/>
      </label>
      <button onClick={() => client.adder.postAdd({a, b}).then(v => setResult(v.result))}>Add A and B</button>
      <button onClick={() => client.subtractor.postSub({a, b}).then(v => setResult(v.result)).catch(e => setError(e.body.message))}>Subtract B from A</button>
      {error ? <h2>Error: {error}</h2> : <h2>Result: {result}</h2>}
    </div>
  );
}

export default App;
