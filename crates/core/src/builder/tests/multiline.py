# A round-trip fixture of the script builder: things a one-line block could lose.
CMD = """show run
show ip int brief"""   # a here-doc command


def outer(a):
    def inner(b):
        """EN: a docstring
        RU: строка документации"""
        return b * 2
    return inner(a)


try:
    with open("/tmp/x") as f, open("/tmp/y", "w") as g:
        g.write(f.read())
except (OSError, ValueError) as e:
    crt.Dialog.MessageBox(str(e))
finally:
    pass

for i in range(3):
    if i == 1:
        continue
    elif i == 2:
        # nothing
        pass
    else:
        crt.Screen.Send(CMD + "\r")
while False:
    pass
