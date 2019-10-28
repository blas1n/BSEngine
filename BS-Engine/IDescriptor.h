#pragma once

#include "Interface.h"

INTERFACE_BEGIN(Descriptor)
	CONST_INTERFACE_DEF(const char*, GetName)
	CONST_INTERFACE_DEF(void*, GetParamsAddress)
	CONST_INTERFACE_DEF(size_t, GetParamsSize)
INTERFACE_END