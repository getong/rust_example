// src/routes/index.tsx
import type { JSX } from "react";
import type { TuonoProps } from "tuono";

import PokemonLink from "@/components/PokemonLink";

interface Pokemon {
  name: string;
}

interface IndexProps {
  results: Array<Pokemon>;
}

export default function IndexPage({
  data,
}: TuonoProps<IndexProps>): JSX.Element | null {
  if (!data?.results) return null;

  return (
    <>
      <title>Tuono tutorial</title>

      <header className="header">
        <a
          href="https://crates.io/crates/tuono"
          target="_blank"
          rel="noreferrer"
        >
          Crates
        </a>
        <a
          href="https://www.npmjs.com/package/tuono"
          target="_blank"
          rel="noreferrer"
        >
          Npm
        </a>
      </header>
      <div className="title-wrap">
        <h1 className="title">
          TU<span>O</span>NO
        </h1>
        <div className="logo">
          <img src="rust.svg" className="rust" />
          <img src="react.svg" className="react" />
        </div>
      </div>
      <ul style={{ flexWrap: "wrap", display: "flex", gap: 10 }}>
        <PokemonLink pokemon={{ name: "GOAT" }} id={0} />

        {data.results.map((pokemon, i) => (
          <PokemonLink key={i} pokemon={pokemon} id={i + 1} />
        ))}
      </ul>
    </>
  );
}
