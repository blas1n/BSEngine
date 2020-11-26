#pragma once

#include "Component.h"

namespace ArenaBoss
{
	class UpdatableComponent : public Component
	{
		GENERATE_COMPONENT1(UpdatableComponent)

	public:
		virtual void Update() = 0;
	};
}