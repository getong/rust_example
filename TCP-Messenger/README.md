# Messenger (TCP chat)

It is a simple Client-Server model that uses TCP to share messages between clients

# Features:

-> Two clients can talk two eachother simultaneously

-> TCP connection is used (only standard lib)

-> One message has a word length of 250 characters

-> Everthing happening here, like the Input from the user, sending input to server, receiving input from server are handled in seperate threads

#  Usage

```
./messenger server 127.0.0.1:9000

./messenger client 127.0.0.1:9000

```

![](https://github.com/michealkeines/TCP-Messenger/blob/main/messenger.gif)
