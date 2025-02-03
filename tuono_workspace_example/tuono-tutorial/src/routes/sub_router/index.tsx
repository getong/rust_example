import { JSX } from "react";
import { useRouter, Link } from "tuono";

export default function IndexPage(): JSX.Element {
  const router = useRouter();

  return (
    <>
      <p>pathname: {router.pathname}</p>

      <div></div>

      <button
        onClick={() => {
          router.push("/"); // Navigate programmatically
        }}
        style={{
          padding: "10px 20px",
          backgroundColor: "#007bff",
          color: "white",
          borderRadius: "5px",
          cursor: "pointer",
          border: "none",
        }}
      >
        <Link href="/" style={{ textDecoration: "none", color: "inherit" }}>
          Home Page
        </Link>
      </button>

      <div></div>

      <Link
        href="/"
        className="button-link" // Add a class for styling
        onClick={(e) => {
          e.preventDefault(); // Prevent default link behavior
          router.push("/"); // Use router.push to navigate
        }}
      >
        Another homepage
      </Link>
    </>
  );
}
