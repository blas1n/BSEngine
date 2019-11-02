#pragma once

#include <list>
#include <forward_list>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using List = std::list<T, Alloc<T>>;

template <class T, template<class>class Alloc = Allocator>
using ForwardList = std::forward_list<T, Alloc<T>>;