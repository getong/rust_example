import clsx from "clsx";

interface ButtonProps {
  isActive: boolean;
}

const Button = ({ isActive }: ButtonProps) => {
  const className = clsx("btn", isActive && "btn-active");

  return <button className={className}>辰火流光</button>;
};

const Demo = () => {
  return (
    <>
      <Button isActive={true} />
      <Button isActive={false} />
    </>
  );
};
export default Demo;
