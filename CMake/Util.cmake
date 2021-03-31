function (register_module)
    get_filename_component (MODULE_NAME ${CMAKE_CURRENT_SOURCE_DIR} NAME)
    
    file (GLOB_RECURSE SRCS "*.cpp")
    add_library (${MODULE_NAME} SHARED ${SRCS})
    target_include_directories (${MODULE_NAME} PUBLIC "Public")

    string (TOUPPER ${MODULE_NAME}_API API)
    target_compile_definitions (${MODULE_NAME} PRIVATE ${API}=${DLL_EXPORT} INTERFACE ${API}=${DLL_IMPORT})

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Public/pch.h")
        target_precompile_headers (${MODULE_NAME} PUBLIC Public/pch.h)
    endif ()

    if (EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/Private/pch.h")
        target_precompile_headers (${MODULE_NAME} PRIVATE Private/pch.h)
    endif ()
endfunction ()

function (download_package name)
    set (cmake_dir "${PROJECT_SOURCE_DIR}/CMake/AutoInstall")
    set (src_dir "${PROJECT_SOURCE_DIR}/Source/ThirdParty/${name}")
    set (build_dir "${CMAKE_BINARY_DIR}/ThirdParty/${name}-build")
    set (download_dir "${CMAKE_BINARY_DIR}/ThirdParty/${name}-download")
    configure_file ("${cmake_dir}/${name}.cmake" "${download_dir}/CMakeLists.txt" @ONLY)
    
    execute_process (
        COMMAND ${CMAKE_COMMAND} -G "${CMAKE_GENERATOR}" .
        RESULT_VARIABLE result
        WORKING_DIRECTORY "${download_dir}"
    )

    if (result)
        message (FATAL_ERROR "CMake step for ${name} failed: ${result}")
    endif ()

    execute_process (
        COMMAND ${CMAKE_COMMAND} --build .
        RESULT_VARIABLE result
        WORKING_DIRECTORY "${download_dir}"
    )

    if (result)
        message (FATAL_ERROR "Build step for ${name} failed: ${result}")
    endif ()

    if (EXISTS "${cmake_dir}/${name}-build.cmake")
         configure_file ("${cmake_dir}/${name}-build.cmake" "${src_dir}/CMakeLists.txt" @ONLY)
    endif ()

    if (EXISTS "${src_dir}/CMakeLists.txt")
         add_subdirectory ("${src_dir}" "${build_dir}" EXCLUDE_FROM_ALL)
    endif ()
endfunction ()

function (get_package name)
    if (${ARGC} GREATER 1)
         set(version ${ARGV1})
    endif ()

    if (version)
        find_package (${name} ${version} QUIET)
    else ()
        find_package (${name} QUIET)
    endif ()
    
    if (${name}_FOUND)
        message (STATUS "Found ${name} from system")
    else ()
        message (STATUS "Could not find ${name} from system. Downloading...")
        download_package (${name})
    endif ()
endfunction ()
