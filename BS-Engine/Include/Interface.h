#pragma once

#include "Macro.h"

#define INTERFACE_BEGIN(name) \
class BS_API I##name abstract { \
public: \
	virtual ~I##name() noexcept = default;

#define INTERFACE_END };

#define INTERFACE_DEF(ret, name, ...) \
virtual ret name(__VA_ARGS__) noexcept abstract;

#define INTERFACE_CONST_DEF(ret, name, ...) \
virtual ret name(__VA_ARGS__) const noexcept abstract;

#define INTERFACE_TYPE_DEF(name, type) \
using name = type;

#define INTERFACE_STRUCT_DEF(name) \
struct name abstract