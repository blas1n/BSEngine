#include "RenderTree.h"
#include <algorithm>
#include <exception>
#include <string>
#include "IteratorFinder.h"
#include "MeshComponent.h"
#include "Shader.h"

namespace ArenaBoss
{
	namespace
	{
		constexpr static auto NodeLess = [](const auto& lhs, const auto& rhs) { return lhs.shader < rhs; };
		constexpr static auto NodeGreater = [](const auto& lhs, const auto& rhs) { return lhs < rhs.shader; };
	}

	void RenderTree::Draw(std::function<void(Shader&)> fn)
	{
		for (auto& node : nodes)
		{
			node.shader->Activate();
			fn(*node.shader);

			for (auto component : node.components)
				if (component->IsVisible())
					component->Draw();
		}
	}

	void RenderTree::RegisterShader(Shader* shader)
	{
		const auto iter = std::upper_bound(nodes.cbegin(), nodes.cend(), shader, NodeGreater);

		if (iter == nodes.cend() || iter->shader != shader)
			nodes.insert(iter, shader);
	}

	void RenderTree::UnregisterShader(Shader* shader)
	{
		const auto iter = std::lower_bound(nodes.cbegin(), nodes.cend(), shader, NodeLess);

		if (iter != nodes.cend() && iter->shader == shader)
			nodes.erase(iter);
	}

	void RenderTree::RegisterComponent(MeshComponent* component)
	{
		auto& components = GetComponents(component->GetShader());
		
		const auto iter = std::upper_bound(components.cbegin(), components.cend(), component);

		if (iter == components.cend() || *iter != component)
			components.insert(iter, component);
	}

	void RenderTree::UnregisterComponent(MeshComponent* component)
	{
		auto& components = GetComponents(component->GetShader());

		const auto iter = std::lower_bound(components.cbegin(), components.cend(), component);

		if (iter != components.cend() && *iter == component)
			components.erase(iter);
	}

	std::vector<MeshComponent*>& RenderTree::GetComponents(Shader* shader)
	{
		const auto iter = std::upper_bound(nodes.begin(), nodes.end(), shader, NodeGreater);

		if (iter != nodes.cend() && iter->shader == shader)
			return iter->components;

		return nodes.insert(iter, shader)->components;
	}
}