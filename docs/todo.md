# Today
So I need to work out these channels.
There's a few things to consider.
- Does an actor need to wait till another actor exists to recv a message
- If not then does the message get buffered? 
- If so how deep, just 1? or n?

So the idea is to have a store of requests and only send a response when the 
sender is online.
What about when a sender comes online with no receiver?
I think that's ok because there will be back pressure unless we use a watch.
The only problem with watch is it needs an inital value.
For inbound mpsc makes sense but wasn't there an issue with creating the channel?
The receiver can't be cloned so needs to be moved out of the hashmap once the channel is established.
Basically once the request has a response.
The start can stay because it can be cloned.

Only Physics online

```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle"]
    S [label="Sound", shape="circle", style="dashed"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    P -> R[label="request channel sound_in_s"]
}
```
Sound comes online and physics is awaiting request
```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle", color="blue" style="dashed"]
    S [label="Sound", shape="circle"]
    R [label="Registry", shape="circle"]
    s_in_s [label="sound_in_s", shape="square"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    R -> s_in_s[color="blue"]
}
```
Now sound needs a way to get sound_i_e
```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle", color="blue" style="dashed"]
    S [label="Sound", shape="circle"]
    R [label="Registry", shape="circle"]
    s_in_s [label="sound_in_s", shape="square"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    R -> s_in_s[color="blue"]
    S -> R[label="request sound_in_e"]
}
```
Registry has full request, both actors awaiting
```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle", color="blue" style="dashed"]
    S [label="Sound", shape="circle", color="blue" style="dashed"]
    R [label="Registry", shape="circle"]
    s_in_s [label="sound_in_s", shape="square"]
    s_in_e [label="sound_in_e", shape="square"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    R -> s_in_s[color="blue"]
    R -> s_in_e[color="blue"]
}
```
Sends back request answers
```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle"]
    S [label="Sound", shape="circle"]
    R [label="Registry", shape="circle"]
    s_in_s [label="sound_in_s", shape="square"]
    s_in_e [label="sound_in_e", shape="square"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    R -> s_in_s -> P[color="orange"]
    R -> s_in_e -> S[color="orange"]
}
```
Now channel is online
```graphviz
digraph A {
    /* Entities */
    P [label="Physics", shape="circle"]
    S [label="Sound", shape="circle"]
    R [label="Registry", shape="circle"]
    R [label="Registry", shape="circle"]

    /* Relationships */
    P -> S[label="sound_in", color="green"]
}
```