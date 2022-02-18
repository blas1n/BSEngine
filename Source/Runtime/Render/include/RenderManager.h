#pragma once

#include "Assertion.h"
#include "Manager.h"

class RENDER_API RenderManager final : public Manager
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	std::shared_ptr<class RHI> GetRHI() noexcept { return rhi; }
	std::shared_ptr<const RHI> GetRHI() const noexcept { return rhi; }

	void SetRHI(std::shared_ptr<RHI> inRhi) noexcept
	{
		if (Ensure(!rhi, "Set RHI must call once."))
			rhi = std::move(inRhi);
	}

private:
	std::shared_ptr<RHI> rhi;
};
