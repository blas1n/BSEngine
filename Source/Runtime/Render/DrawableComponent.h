#pragma once

#include "Accessor.h"
#include "Component.h"

namespace ArenaBoss
{
	class Shader;

	class DrawableComponent : public Component
	{
		GENERATE_COMPONENT1(DrawableComponent)

	public:
		virtual void Draw() = 0;
		
		inline Shader* GetShader() noexcept { return shader; }
		inline const Shader* GetShader() const noexcept { return shader; }
		inline virtual void SetShader(Shader* inShader) noexcept { shader = inShader; }

		inline bool HaveShader() const noexcept { return shader != nullptr; }
		inline void ClearShader() noexcept { SetShader(nullptr); }

		inline bool IsVisible() const noexcept { return visible; }
		inline void SetVisible(bool inVisible) noexcept { visible = inVisible; }

	private:
		Shader* shader = nullptr;
		bool visible = true;
	};
}