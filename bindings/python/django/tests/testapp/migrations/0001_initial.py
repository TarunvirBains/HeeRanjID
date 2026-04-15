from django.db import migrations, models
import uuid
import django.utils.timezone
import heeranjid_django.fields


class Migration(migrations.Migration):

    initial = True

    dependencies = [
        ("heeranjid_django", "0001_install_heeranjid"),
    ]

    operations = [
        migrations.CreateModel(
            name="VanillaPost",
            fields=[
                ("id", models.UUIDField(primary_key=True, default=uuid.uuid4, editable=False, serialize=False)),
                ("title", models.CharField(max_length=255)),
                ("created_at", models.DateTimeField(auto_now_add=True)),
            ],
            options={
                "ordering": ["created_at"],
                "app_label": "testapp",
            },
        ),
        migrations.CreateModel(
            name="HeeRanjPost",
            fields=[
                ("id", heeranjid_django.fields.RanjIdField(primary_key=True, serialize=False)),
                ("title", models.CharField(max_length=255)),
                ("created_at", models.DateTimeField(auto_now_add=True)),
            ],
            options={
                "ordering": ["id"],
                "app_label": "testapp",
            },
        ),
    ]
