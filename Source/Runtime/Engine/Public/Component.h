#pragma once

#include "Name.h"

class ENGINE_API Component
{

};

template <class T = Component>
T* CreateComponent(Name name)
{
    return reinterpret_cast<T*>(Impl::CreateComponent(name));
}

namespace Impl
{
    template <class T>
    Component* Create()
    {
        return new T{};
    }

    Component* CreateComponent(Name name);
    void RegisterComponent(Name name, Component*(*ptr)());
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
