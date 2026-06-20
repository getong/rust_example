def example(*args, **kwargs) -> None:
    if args != () and kwargs != {}:
        print("called with args and kwargs", args, kwargs)
    elif args != ():
        print("called with args", args)
    elif kwargs != {}:
        print("called with kwargs", kwargs)
    else:
        print("called with no arguments")
