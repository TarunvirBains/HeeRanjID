use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeerId {
    inner: heeranjid::HeerId,
}

#[pymethods]
impl HeerId {
    #[new]
    fn py_new(value: i64) -> PyResult<Self> {
        let inner = heeranjid::HeerId::from_i64(value)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::HeerId = s.parse().map_err(|e: heeranjid::Error| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        Ok(Self { inner })
    }

    fn as_int(&self) -> i64 {
        self.inner.as_i64()
    }

    #[getter]
    fn timestamp_ms(&self) -> u64 {
        self.inner.timestamp_ms()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("HeerId({})", self.inner.as_i64())
    }

    #[staticmethod]
    fn batch_to_ranjids(ids: Vec<HeerId>) -> PyResult<Vec<(HeerId, RanjId)>> {
        let rust_ids: Vec<heeranjid::HeerId> = ids.iter().map(|h| h.inner).collect();
        let pairs = heeranjid::HeerId::batch_to_ranjids(&rust_ids);
        Ok(pairs
            .into_iter()
            .map(|(h, r)| (HeerId { inner: h }, RanjId { inner: r }))
            .collect())
    }

    #[staticmethod]
    fn check_ranjid_convertibility(_ids: Vec<HeerId>) -> Vec<String> {
        // Always empty — HeerId always fits in RanjId
        Vec::new()
    }
}

#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RanjId {
    inner: heeranjid::RanjId,
}

#[pymethods]
impl RanjId {
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::RanjId = s.parse().map_err(|e: heeranjid::Error| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        Ok(Self { inner })
    }

    fn to_uuid<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let uuid_mod = py.import("uuid")?;
        let uuid_cls = uuid_mod.getattr("UUID")?;
        let s = self.inner.as_uuid().to_string();
        uuid_cls.call1((s,))
    }

    #[getter]
    fn timestamp_micros(&self) -> u128 {
        self.inner.timestamp_micros()
    }

    #[getter]
    fn timestamp(&self) -> u128 {
        self.inner.timestamp()
    }

    #[getter]
    fn precision(&self) -> String {
        self.inner.precision().label().to_string()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("RanjId({})", self.inner.as_uuid())
    }

    #[staticmethod]
    fn batch_to_heerids(ids: Vec<RanjId>) -> PyResult<Vec<(RanjId, HeerId)>> {
        let rust_ids: Vec<heeranjid::RanjId> = ids.iter().map(|r| r.inner).collect();
        match heeranjid::RanjId::batch_to_heerids(&rust_ids) {
            Ok(pairs) => Ok(pairs
                .into_iter()
                .map(|(r, h)| (RanjId { inner: r }, HeerId { inner: h }))
                .collect()),
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
        }
    }

    #[staticmethod]
    fn check_heerid_convertibility(ids: Vec<RanjId>) -> Vec<String> {
        let rust_ids: Vec<heeranjid::RanjId> = ids.iter().map(|r| r.inner).collect();
        heeranjid::RanjId::check_heerid_convertibility(&rust_ids)
            .into_iter()
            .map(|c| format!("{:?}", c))
            .collect()
    }
}

#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeerIdDesc {
    inner: heeranjid::HeerIdDesc,
}

#[pymethods]
impl HeerIdDesc {
    #[new]
    fn py_new(value: i64) -> PyResult<Self> {
        let inner = heeranjid::HeerIdDesc::from_i64(value)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::HeerIdDesc = s.parse().map_err(|e: heeranjid::Error| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_parts(timestamp_ms: u64, node_id: u16, sequence: u16) -> PyResult<Self> {
        let inner = heeranjid::HeerIdDesc::new(timestamp_ms, node_id, sequence)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn as_int(&self) -> i64 {
        self.inner.as_i64()
    }

    #[getter]
    fn timestamp_ms(&self) -> u64 {
        self.inner.timestamp_ms()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("HeerIdDesc({})", self.inner.as_i64())
    }
}

#[pyclass(frozen, eq, ord, hash)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RanjIdDesc {
    inner: heeranjid::RanjIdDesc,
}

#[pymethods]
impl RanjIdDesc {
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        let inner: heeranjid::RanjIdDesc = s.parse().map_err(|e: heeranjid::Error| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        Ok(Self { inner })
    }

    #[staticmethod]
    fn from_parts(timestamp: u128, precision: &str, node_id: u16, sequence: u16) -> PyResult<Self> {
        let prec = match precision {
            "microseconds" | "us" => heeranjid::RanjPrecision::Microseconds,
            "nanoseconds" | "ns" => heeranjid::RanjPrecision::Nanoseconds,
            "picoseconds" | "ps" => heeranjid::RanjPrecision::Picoseconds,
            "femtoseconds" | "fs" => heeranjid::RanjPrecision::Femtoseconds,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unknown precision {other:?}; expected one of: microseconds/us, nanoseconds/ns, picoseconds/ps, femtoseconds/fs"
                )));
            }
        };
        let inner = heeranjid::RanjIdDesc::new(timestamp, prec, node_id, sequence)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    fn to_uuid<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let uuid_mod = py.import("uuid")?;
        let uuid_cls = uuid_mod.getattr("UUID")?;
        let s = self.inner.as_uuid().to_string();
        uuid_cls.call1((s,))
    }

    #[getter]
    fn timestamp(&self) -> u128 {
        self.inner.timestamp()
    }

    #[getter]
    fn precision(&self) -> String {
        self.inner.precision().label().to_string()
    }

    #[getter]
    fn node_id(&self) -> u16 {
        self.inner.node_id()
    }

    #[getter]
    fn sequence(&self) -> u16 {
        self.inner.sequence()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("RanjIdDesc({})", self.inner.as_uuid())
    }
}

// --- mssql_schema submodule -------------------------------------------

#[pyfunction]
#[pyo3(signature = (table, pairs, kind))]
fn mssql_install_autofill_trigger_for_table(
    table: &str,
    pairs: Vec<(String, String)>,
    kind: &str,
) -> PyResult<String> {
    use heeranjid::mssql_schema as ms;
    use heeranjid::schema_shared::{ColumnPair, IdKind};
    let kind = match kind {
        "heer" => IdKind::Heer,
        "ranj" => IdKind::Ranj,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown kind: {other}; expected 'heer' or 'ranj'"
            )));
        }
    };
    let cp: Vec<ColumnPair<'_>> = pairs
        .iter()
        .map(|(s, d)| ColumnPair { src: s, dst: d })
        .collect();
    ms::install_autofill_trigger_for_table_mssql(table, &cp, kind)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn mssql_drop_autofill_trigger_for_table(table: &str) -> PyResult<String> {
    heeranjid::mssql_schema::drop_autofill_trigger_for_table_mssql(table)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

fn register_mssql_schema_submodule<'py>(
    py: Python<'py>,
    parent: &Bound<'py, PyModule>,
) -> PyResult<()> {
    let m = PyModule::new(py, "mssql_schema")?;
    m.add("DESC_FLIP_TSQL", heeranjid::mssql_schema::DESC_FLIP_TSQL)?;
    m.add(
        "DESC_GENERATORS_TSQL",
        heeranjid::mssql_schema::DESC_GENERATORS_TSQL,
    )?;
    m.add(
        "BULK_BACKFILL_TSQL",
        heeranjid::mssql_schema::BULK_BACKFILL_TSQL,
    )?;
    m.add(
        "INSTALL_ALL_DESC_TSQL",
        heeranjid::mssql_schema::INSTALL_ALL_DESC_TSQL,
    )?;
    m.add_function(wrap_pyfunction!(
        mssql_install_autofill_trigger_for_table,
        &m
    )?)?;
    m.add_function(wrap_pyfunction!(mssql_drop_autofill_trigger_for_table, &m)?)?;
    parent.add_submodule(&m)?;
    Ok(())
}

#[pymodule]
fn _heeranjid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HeerId>()?;
    m.add_class::<RanjId>()?;
    m.add_class::<HeerIdDesc>()?;
    m.add_class::<RanjIdDesc>()?;
    register_mssql_schema_submodule(m.py(), m)?;
    Ok(())
}
