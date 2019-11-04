#pragma once

#include "IManager.h"

#define ManagerVariable(Class, Name) \
private: \
Class * Name; \
friend ManagerManager; \
void Set##Name(Class * In##Name) noexcept \
{ \
	Name = In##Name; \
}

template <uint8 Idx>
struct ManagerIndexType
{
	using Index = Idx;
};

template <uint8 Idx>
using ManagerIndex = typename ManagerIndexType<Idx>::Index;

#define ManagerInjection(Injected, Inject) \
constexpr bool IsInject(ManagerIndex<Injected::GetIndex()> injectedIndex, \
	ManagerIndex<Inject::GetIndex()> injectIndex) noexcept \
{ \
	return true; \
}

class BS_API ManagerManager : public IManager
{
};