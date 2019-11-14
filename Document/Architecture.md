# BS-Engine Architecture

## Architecture (Simple)

![Architecture](./Image/Architecture.png)

## Architecture (Detail)

### Library

This layer contains math, graphics, physics, input, etc. It wrap or contain external API and make basic API. It is platform-independent. It must guarantee to stateless. And it is reusable  

### Managers

This layer contains rendering, input, physics, etc. Manager has method, initializing and releasing myself and handling request of the object. It must guarantee to unique.  

### System

This layer contains a system that is a singleton object. Take responsibility for initializing, updating, releasing the scene and managers. Objects accesses manager through this. It also manages the beginning and end of the game.

### Scene

Scene take charge for updating objects. It also creates and deletes included objects when enter or leave.

### Object

The object is simply responsible for creating, deleting, and updating components. It also helps managers and other objects to access own components.

### Component

Component is responsible for working with objects that are empty shells. This interacts with the manager to manipulate the game.
