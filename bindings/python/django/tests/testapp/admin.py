from django.contrib import admin

from .models import HeeRanjPost, VanillaPost


@admin.register(VanillaPost)
class VanillaPostAdmin(admin.ModelAdmin):
    list_display = ["id", "title", "created_at"]
    readonly_fields = ["id", "created_at"]


@admin.register(HeeRanjPost)
class HeeRanjPostAdmin(admin.ModelAdmin):
    list_display = ["id", "title", "created_at"]
    readonly_fields = ["id", "created_at"]
