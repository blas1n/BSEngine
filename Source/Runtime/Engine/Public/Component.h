#pragma once

#include "Name.h"
#include "Json.h"

class Entity;

class ENGINE_API Component
{
public:
    Component(Entity* inEntity) : entity(inEntity) {}
    virtual ~Component() {};

    virtual Json Serialize() const { return Json{}; }
    virtual void Deserialize(const Json& json) {}

    Entity* GetEntity() noexcept { return entity; }
    const Entity* GetEntity() const noexcept { return entity; }

private:
    Entity* entity;
};

template <class T = Component>
T* CreateComponent(Name name, Entity* entity)
{
    return reinterpret_cast<T*>(Impl::CreateComponent(name, entity));
}

namespace Impl
{
    template <class T>
    Component* Create(Entity* entity)
    {
        return new T{ entity };
    }

    Component* CreateComponent(Name name, Entity* entity);
    void RegisterComponent(Name name, Component*(*ptr)(Entity*));
}

#define REGISTER_COMPONENT(Class)                                                   \
static void _RegisterReflectionComponent();                                         \
namespace                                                                           \
{                                                                                   \
    struct _ComponentRegister                                                       \
    {                                                                               \
        _ComponentRegister()                                                        \
        {                                                                           \
            _RegisterReflectionComponent();                                         \
        }                                                                           \
    };                                                                              \
}                                                                                   \
static const _ComponentRegister ADD_PREFIX(ComponentRegister_, __LINE__);           \
static void _RegisterReflectionComponent()                                          \
{                                                                                   \
    Impl::RegisterComponent(Name{ STR(#Class) }, &Impl::Create<Class>);             \
}
