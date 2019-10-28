#pragma once

#include "Interface.h"

INTERFACE_BEGIN(Descriptor)
	INTERFACE_CONST_DEF(const char*, GetName)
	INTERFACE_CONST_DEF(void*, GetParamsAddress)
	INTERFACE_CONST_DEF(size_t, GetParamsSize)
INTERFACE_END