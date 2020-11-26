#include "ResourceManager.h"
#include <exception>
#include "Resource.h"

namespace ArenaBoss
{
	void ResourceManager::DeleteResource(const std::string& name)
	{
		const auto iter = IteratorFinder::FindSameIterator(resources, name);
		delete *iter;
		resources.erase(iter);
	}

	void ResourceManager::RegisterResource(Resource* resource)
	{
		const auto iter = std::upper_bound(
			resources.cbegin(),
			resources.cend(),
			resource->GetName(),
			[](const auto& lhs, const auto& rhs) { return lhs < *rhs; }
		);
		resources.insert(iter, resource);
	}

	Resource* ResourceManager::FindResource(const std::string& name, const std::string& resourceName)
	{
		const auto resource = *IteratorFinder::FindSameIterator(resources, name);
		if (resource->ClassName() != resourceName)
			throw std::exception{ "No corresponding resource exists." };

		return resource;
	}
}