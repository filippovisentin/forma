# Install script for directory: /home/user/forma/third_party/opennurbs

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Install shared libraries without execute permission?
if(NOT DEFINED CMAKE_INSTALL_SO_NO_EXE)
  set(CMAKE_INSTALL_SO_NO_EXE "1")
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

# Set default install directory permissions.
if(NOT DEFINED CMAKE_OBJDUMP)
  set(CMAKE_OBJDUMP "/usr/bin/objdump")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for the subdirectory.
  include("/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/android_uuid/cmake_install.cmake")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for the subdirectory.
  include("/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/freetype263/cmake_install.cmake")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for the subdirectory.
  include("/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/zlib/cmake_install.cmake")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/libopennurbsStatic.a")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/CMakeFiles/opennurbsStatic.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/opennurbsStatic" TYPE FILE FILES
    "/home/user/forma/third_party/opennurbs/opennurbs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_attributes.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_properties.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_settings.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_annotationbase.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_apple_nsfont.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_arc.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_arccurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_archivable_dictionary.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_archive.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_array.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_array_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_base32.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_base64.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_beam.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bezier.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bitmap.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bounding_box.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_box.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_brep.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_circle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_color.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_compress.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_compstat.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_cone.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_convex_poly.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_crc.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curveonsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curveproxy.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_cylinder.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_date.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_decals.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_defines.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_detail.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimension.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimensionformat.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimensionstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dithering.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ellipse.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_embedded_file.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_error.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_evaluate_nurbs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_extensions.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_file_utilities.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_font.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fpoint.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_freetype.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_freetype_include.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fsp.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fsp_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_function_list.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_geometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_gl.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ground_plane.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_group.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hash_table.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hatch.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hsort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_instance.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V2_annotation.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V5_annotation.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V5_dimstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_defines.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_glyph.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_intersect.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ipoint.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_knot.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_layer.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_leader.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_light.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_line.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linear_workflow.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linestyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linetype.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_locale.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_lock.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_lookup.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mapchan.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_material.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_math.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_matrix.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_md5.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_memory.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mesh.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mesh_modifiers.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_model_component.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_model_geometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_nurbscurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_nurbssurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_object.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_object_history.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_objref.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_offsetsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_optimize.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_parse.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_photogrammetry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_plane.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_planesurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pluginlist.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_point.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointcloud.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointgeometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointgrid.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polycurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polyedgecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polyline.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polylinecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_post_effects.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_private_wrap.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_private_wrap_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_progress_reporter.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_qsort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_quacksort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_quaternion.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rand.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_render_channels.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_render_content.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rendering.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_revsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rtree.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_safe_frame.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sectionstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sha1.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_skylight.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sleeplock.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sphere.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_std_string.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_string.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_string_value.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_subd.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_subd_data.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sumsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sun.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_surface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_surfaceproxy.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_symmetry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system_compiler.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system_runtime.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_terminator.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_testclass.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_text.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_text_style.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textcontext.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textdraw.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textglyph.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textiterator.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textlog.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textobject.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textrun.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_texture.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_texture_mapping.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_topology.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_torus.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_unicode.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_userdata.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_uuid.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_version.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_version_number.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_viewport.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_wip.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_workspace.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_xform.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_xml.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_zlib.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so"
         RPATH "")
  endif()
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE SHARED_LIBRARY FILES "/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/libOpenNURBS.so")
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/usr/bin/strip" "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libOpenNURBS.so")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/CMakeFiles/OpenNURBS.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/OpenNURBS" TYPE FILE FILES
    "/home/user/forma/third_party/opennurbs/opennurbs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_attributes.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_properties.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_3dm_settings.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_annotationbase.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_apple_nsfont.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_arc.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_arccurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_archivable_dictionary.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_archive.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_array.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_array_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_base32.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_base64.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_beam.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bezier.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bitmap.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_bounding_box.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_box.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_brep.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_circle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_color.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_compress.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_compstat.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_cone.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_convex_poly.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_crc.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curveonsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_curveproxy.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_cylinder.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_date.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_decals.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_defines.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_detail.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimension.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimensionformat.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dimensionstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_dithering.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ellipse.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_embedded_file.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_error.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_evaluate_nurbs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_extensions.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_file_utilities.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_font.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fpoint.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_freetype.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_freetype_include.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fsp.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_fsp_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_function_list.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_geometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_gl.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ground_plane.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_group.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hash_table.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hatch.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_hsort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_instance.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V2_annotation.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V5_annotation.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_V5_dimstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_defines.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_internal_glyph.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_intersect.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_ipoint.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_knot.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_layer.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_leader.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_light.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_line.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linear_workflow.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linestyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_linetype.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_locale.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_lock.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_lookup.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mapchan.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_material.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_math.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_matrix.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_md5.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_memory.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mesh.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_mesh_modifiers.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_model_component.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_model_geometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_nurbscurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_nurbssurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_object.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_object_history.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_objref.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_offsetsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_optimize.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_parse.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_photogrammetry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_plane.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_planesurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pluginlist.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_point.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointcloud.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointgeometry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_pointgrid.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polycurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polyedgecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polyline.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_polylinecurve.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_post_effects.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_private_wrap.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_private_wrap_defs.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_progress_reporter.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_qsort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_quacksort_template.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_quaternion.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rand.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_render_channels.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_render_content.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rendering.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_revsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_rtree.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_safe_frame.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sectionstyle.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sha1.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_skylight.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sleeplock.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sphere.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_std_string.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_string.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_string_value.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_subd.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_subd_data.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sumsurface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_sun.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_surface.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_surfaceproxy.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_symmetry.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system_compiler.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_system_runtime.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_terminator.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_testclass.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_text.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_text_style.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textcontext.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textdraw.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textglyph.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textiterator.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textlog.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textobject.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_textrun.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_texture.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_texture_mapping.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_topology.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_torus.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_unicode.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_userdata.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_uuid.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_version.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_version_number.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_viewport.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_wip.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_workspace.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_xform.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_xml.h"
    "/home/user/forma/third_party/opennurbs/opennurbs_zlib.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT)
  set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INSTALL_COMPONENT}.txt")
else()
  set(CMAKE_INSTALL_MANIFEST "install_manifest.txt")
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
file(WRITE "/home/user/forma/target-occt/debug/build/forma-io-3dm-a64dca0dd1ff1025/out/build/${CMAKE_INSTALL_MANIFEST}"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
