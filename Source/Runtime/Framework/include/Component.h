#pragma once

#include "Name.h"
#include "Json.h"

class Entity;

class FRAMEWORK_API Component
{
public:
    Component(Entity* inEntity) : entity(inEntity) {}
    virtual ~Component() {};

    [[nodiscard]] virtual Json Serialize() const { return Json{}; }
    virtual void Deserialize(const Json& json) {}

    [[nodiscard]] Entity* GetEntity() noexcept { return entity; }
    [[nodiscard]] const Entity* GetEntity() const noexcept { return entity; }

private:
    Entity* entity;
};

template <class T = Component>
[[nodiscard]] T* CreateComponent(Name name, Entity* entity)
{
    return reinterpret_cast<T*>(Impl::CreateComponent(name, entity));
}

namespace Impl
{
    template <class T>
    [[nodiscard]] Component* Create(Entity* entity)
    {
        return new T{ entity };
    }

    [[nodiscard]] FRAMEWORK_API Component* CreateComponent(Name name, Entity* entity);
    FRAMEWORK_API void RegisterComponent(Name name, Component*(*ptr)(Entity*));
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
