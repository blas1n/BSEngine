#pragma once

#include <vector>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using Array = std::vector<T, Alloc<T>>;